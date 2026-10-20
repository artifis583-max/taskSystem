//! Every user action as one function. Each write runs inside a SQLite transaction.
//! `today` is always passed in so tests can control the date.

use crate::domain::{self, Schedule, Size};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    /// A rule said no. The message is shown to the user as-is.
    #[error("{0}")]
    Rule(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

pub(crate) fn rule<T>(msg: &str) -> Result<T> {
    Err(AppError::Rule(msg.to_string()))
}

const DATE_FMT: &str = "%Y-%m-%d";

pub(crate) fn ds(d: NaiveDate) -> String {
    d.format(DATE_FMT).to_string()
}

pub(crate) fn pd(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, DATE_FMT).map_err(|_| AppError::Rule(format!("Bad date: {s}")))
}

pub(crate) fn now_utc() -> String {
    Utc::now().to_rfc3339()
}

// ---------- types shared with the UI ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub title: String,
    #[serde(default)]
    pub details: String,
    pub size: Size,
    pub schedule: Schedule,
    pub start_date: Option<String>,
    /// Last day a repeating task appears (inclusive). None = no end. Ignored for one-time tasks.
    #[serde(default)]
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCard {
    pub occurrence_id: i64,
    pub task_id: i64,
    pub title: String,
    pub details: String,
    pub size: Size,
    pub status: String,
    pub rollover_count: i64,
    /// Coins awarded (if completed) or coins you'd get now (if pending).
    pub coins: i64,
    pub xp: i64,
    pub completed_seq: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextLevel {
    pub level: i64,
    pub xp: i64,
    pub streak: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub today: String,
    pub balance: i64,
    pub xp: i64,
    pub streak: i64,
    pub level: i64,
    pub next_level: Option<NextLevel>,
    /// Pending first (user order), then completed (oldest → newest).
    pub cards: Vec<TaskCard>,
    pub revives: crate::shop::ReviveInfo,
    pub rewards: Vec<crate::shop::Reward>,
    /// Most recent first.
    pub redemptions: Vec<crate::shop::Redemption>,
    pub upcoming: Vec<crate::manage::UpcomingDay>,
    pub settings: crate::settings::Settings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    pub coins: i64,
    pub xp: i64,
    pub balance: i64,
    pub level_before: i64,
    pub level_after: i64,
}

// ---------- small queries ----------

pub(crate) fn balance(c: &Connection) -> Result<i64> {
    Ok(c.query_row("SELECT COALESCE(SUM(coins),0) FROM ledger", [], |r| r.get(0))?)
}

pub(crate) fn total_xp(c: &Connection) -> Result<i64> {
    Ok(c.query_row("SELECT COALESCE(SUM(xp),0) FROM ledger", [], |r| r.get(0))?)
}

pub(crate) fn current_streak(c: &Connection) -> Result<i64> {
    Ok(c
        .query_row("SELECT streak_after FROM days ORDER BY date DESC LIMIT 1", [], |r| r.get(0))
        .optional()?
        .unwrap_or(0))
}

fn last_finalized(c: &Connection) -> Result<Option<NaiveDate>> {
    let s: Option<String> = c.query_row("SELECT MAX(date) FROM days", [], |r| r.get(0))?;
    s.map(|s| pd(&s)).transpose()
}

fn base_rewards(size: &str, coin_override: Option<i64>, xp_override: Option<i64>) -> (i64, i64) {
    let (c, x) = Size::parse(size).unwrap_or(Size::Small).base();
    (coin_override.unwrap_or(c), xp_override.unwrap_or(x))
}

pub(crate) fn op_seen(c: &Connection, op_id: &str) -> Result<bool> {
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM ledger WHERE op_id = ?1)", [op_id], |r| r.get(0))?)
}

pub(crate) fn check_op_id(op_id: &str) -> Result<()> {
    if op_id.is_empty() || op_id.len() > 64 {
        return rule("Invalid operation id.");
    }
    Ok(())
}

// ---------- day processing ----------

/// Create occurrences due on `d` for active tasks. A task never gets a second
/// open occurrence, and never gets a new one on a day it already has one due.
fn generate(tx: &Transaction, d: NaiveDate) -> Result<()> {
    let day = ds(d);
    let mut stmt = tx.prepare(
        "SELECT id, schedule_json, start_date, end_date FROM tasks WHERE status = 'active'",
    )?;
    let tasks: Vec<(i64, String, String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, sched, start, end) in tasks {
        let Ok(schedule) = serde_json::from_str::<Schedule>(&sched) else { continue };
        let end = end.as_deref().map(pd).transpose()?;
        if !schedule.occurs_on(pd(&start)?, end, d) {
            continue;
        }
        let blocked: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM occurrences WHERE task_id = ?1 AND (status = 'pending' OR due_date = ?2))",
            params![id, day],
            |r| r.get(0),
        )?;
        if !blocked {
            tx.execute(
                "INSERT OR IGNORE INTO occurrences (task_id, original_date, due_date) VALUES (?1, ?2, ?2)",
                params![id, day],
            )?;
        }
    }
    Ok(())
}

/// Close out a past day: score it, update the streak, roll unfinished tasks to the next day.
fn finalize_day(tx: &Transaction, d: NaiveDate) -> Result<()> {
    let day = ds(d);
    let done: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM days WHERE date = ?1)", [&day], |r| r.get(0))?;
    if done {
        return Ok(());
    }
    generate(tx, d)?;
    let (eligible, completed): (i64, i64) = tx.query_row(
        "SELECT COUNT(*), COALESCE(SUM(status = 'completed'),0) FROM occurrences
         WHERE due_date = ?1 AND status IN ('pending','completed','skipped')",
        [&day],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let result = domain::day_result(eligible, completed);
    let streak = domain::next_streak(current_streak(tx)?, result);
    tx.execute(
        "UPDATE occurrences SET due_date = ?2, rollover_count = rollover_count + 1
         WHERE due_date = ?1 AND status = 'pending'",
        params![day, ds(d.succ_opt().expect("date overflow"))],
    )?;
    tx.execute(
        "INSERT INTO days (date, eligible, completed, result, streak_after, finalized_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![day, eligible, completed, result.as_str(), streak, now_utc()],
    )?;
    Ok(())
}

/// Bring the database up to `today`: finalize every unfinished past day in order
/// (each in its own transaction), then generate today's tasks.
/// If the clock went backwards, nothing is finalized or generated.
pub fn sync_day(conn: &mut Connection, today: NaiveDate) -> Result<()> {
    let first: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = 'first_day'", [], |r| r.get(0))
        .optional()?;
    let first = match first {
        Some(s) => pd(&s)?,
        None => {
            conn.execute("INSERT INTO settings (key, value) VALUES ('first_day', ?1)", [ds(today)])?;
            today
        }
    };
    let last = last_finalized(conn)?;
    if last.is_some_and(|l| today <= l) {
        return Ok(()); // clock moved back: leave finalized history alone
    }
    let mut d = last.map(|l| l.succ_opt().unwrap()).unwrap_or(first);
    while d < today {
        let tx = conn.transaction()?;
        finalize_day(&tx, d)?;
        tx.commit()?;
        d = d.succ_opt().unwrap();
    }
    let tx = conn.transaction()?;
    generate(&tx, today)?;
    tx.commit()?;
    Ok(())
}

// ---------- reads ----------

pub fn get_state(conn: &mut Connection, today: NaiveDate) -> Result<AppState> {
    sync_day(conn, today)?;
    let xp = total_xp(conn)?;
    let streak = current_streak(conn)?;
    let level = domain::level_for(xp, streak);
    let mut stmt = conn.prepare(
        "SELECT o.id, t.id, t.title, t.details, t.size, o.status, o.rollover_count,
                o.awarded_coins, o.awarded_xp, o.completed_seq, t.coin_override, t.xp_override
         FROM occurrences o JOIN tasks t ON t.id = o.task_id
         WHERE o.due_date = ?1 AND o.status IN ('pending','completed')
         ORDER BY (o.status = 'completed'), o.completed_seq, t.sort_order, o.id",
    )?;
    let cards = stmt
        .query_map([ds(today)], |r| {
            let size: String = r.get(4)?;
            let status: String = r.get(5)?;
            let rollover: i64 = r.get(6)?;
            let (base_c, base_x) = base_rewards(&size, r.get(10)?, r.get(11)?);
            let (coins, xp) = if status == "completed" {
                (r.get::<_, Option<i64>>(7)?.unwrap_or(0), r.get::<_, Option<i64>>(8)?.unwrap_or(0))
            } else {
                (domain::coins_for(base_c, level, rollover), base_x)
            };
            Ok(TaskCard {
                occurrence_id: r.get(0)?,
                task_id: r.get(1)?,
                title: r.get(2)?,
                details: r.get(3)?,
                size: Size::parse(&size).unwrap_or(Size::Small),
                status,
                rollover_count: rollover,
                coins,
                xp,
                completed_seq: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(AppState {
        today: ds(today),
        balance: balance(conn)?,
        xp,
        streak,
        level,
        next_level: domain::next_level(level).map(|(level, xp, streak)| NextLevel { level, xp, streak }),
        cards,
        revives: crate::shop::revive_info(conn, today)?,
        rewards: crate::shop::list_rewards(conn)?,
        redemptions: crate::shop::recent_redemptions(conn, 5)?,
        upcoming: crate::manage::upcoming(conn, today, 7)?,
        settings: crate::settings::load(conn)?,
    })
}

// ---------- writes ----------

/// Checked task fields, ready to store.
pub(crate) struct ValidTask {
    pub title: String,
    pub details: String,
    pub schedule_json: String,
    pub start: NaiveDate,
    pub end: Option<NaiveDate>,
}

/// `old_start` / `old_end` are the task's current dates when editing; keeping them is always allowed.
pub(crate) fn validate_task(
    t: &NewTask,
    today: NaiveDate,
    old_start: Option<NaiveDate>,
    old_end: Option<NaiveDate>,
) -> Result<ValidTask> {
    let title = t.title.trim();
    let len = title.chars().count();
    if len == 0 || len > 120 {
        return rule("Title must be 1–120 characters.");
    }
    let details = t.details.trim();
    if details.chars().count() > 500 {
        return rule("Details must be 500 characters or fewer.");
    }
    if !t.schedule.is_valid() {
        return rule("That repeat setting isn't valid.");
    }
    let start = match &t.start_date {
        Some(s) => pd(s)?,
        None => old_start.unwrap_or(today),
    };
    if start < today && Some(start) != old_start {
        return rule("Start date can't be in the past.");
    }
    if (start - today).num_days() > 5 * 366 {
        return rule("Start date is too far away.");
    }
    let end = match (&t.schedule, &t.end_date) {
        (Schedule::Once, _) | (_, None) => None,
        (_, Some(s)) => Some(pd(s)?),
    };
    if let Some(e) = end {
        if e < start {
            return rule("End date can't be before the start date.");
        }
        if e < today && Some(e) != old_end {
            return rule("End date can't be in the past.");
        }
    }
    Ok(ValidTask {
        title: title.to_string(),
        details: details.to_string(),
        schedule_json: serde_json::to_string(&t.schedule).expect("schedule serializes"),
        start,
        end,
    })
}

pub fn create_task(conn: &mut Connection, today: NaiveDate, t: NewTask) -> Result<i64> {
    let v = validate_task(&t, today, None, None)?;
    let id = {
        let tx = conn.transaction()?;
        let order: i64 = tx.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM tasks", [], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO tasks (title, details, size, schedule_json, start_date, end_date, sort_order, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![v.title, v.details, t.size.as_str(), v.schedule_json, ds(v.start), v.end.map(ds), order, now_utc()],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        id
    };
    sync_day(conn, today)?;
    Ok(id)
}

pub fn complete_occurrence(conn: &mut Connection, today: NaiveDate, op_id: &str, occ_id: i64) -> Result<CompleteResult> {
    check_op_id(op_id)?;
    sync_day(conn, today)?;
    let tx = conn.transaction()?;
    if op_seen(&tx, op_id)? {
        // Same click delivered twice: report what was already recorded.
        let (coins, xp): (i64, i64) =
            tx.query_row("SELECT coins, xp FROM ledger WHERE op_id = ?1", [op_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let lvl = domain::level_for(total_xp(&tx)?, current_streak(&tx)?);
        return Ok(CompleteResult { coins, xp, balance: balance(&tx)?, level_before: lvl, level_after: lvl });
    }
    let row: Option<(String, String, i64, String, Option<i64>, Option<i64>)> = tx
        .query_row(
            "SELECT o.status, o.due_date, o.rollover_count, t.size, t.coin_override, t.xp_override
             FROM occurrences o JOIN tasks t ON t.id = o.task_id WHERE o.id = ?1",
            [occ_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    let Some((status, due, rollover, size, co, xo)) = row else { return rule("Task not found.") };
    if status != "pending" {
        return rule("This task is already done.");
    }
    if due != ds(today) {
        return rule("This task isn't due today.");
    }
    let (base_c, base_x) = base_rewards(&size, co, xo);
    let xp_before = total_xp(&tx)?;
    let streak = current_streak(&tx)?;
    let level_before = domain::level_for(xp_before, streak);
    let coins = domain::coins_for(base_c, level_before, rollover);
    let xp = base_x;
    let seq: i64 = tx.query_row("SELECT COALESCE(MAX(completed_seq),0)+1 FROM occurrences", [], |r| r.get(0))?;
    let now = now_utc();
    tx.execute(
        "UPDATE occurrences SET status='completed', completed_at=?2, awarded_coins=?3, awarded_xp=?4, completed_seq=?5 WHERE id=?1",
        params![occ_id, now, coins, xp, seq],
    )?;
    tx.execute(
        "INSERT INTO ledger (op_id, kind, coins, xp, occurrence_id, created_at) VALUES (?1,'task',?2,?3,?4,?5)",
        params![op_id, coins, xp, occ_id, now],
    )?;
    let result = CompleteResult {
        coins,
        xp,
        balance: balance(&tx)?,
        level_before,
        level_after: domain::level_for(xp_before + xp, streak),
    };
    tx.commit()?;
    Ok(result)
}

/// Undo a completion from today. Refused if the coins were already spent.
pub fn uncomplete_occurrence(conn: &mut Connection, today: NaiveDate, op_id: &str, occ_id: i64) -> Result<i64> {
    check_op_id(op_id)?;
    sync_day(conn, today)?;
    let tx = conn.transaction()?;
    if op_seen(&tx, op_id)? {
        return balance(&tx);
    }
    let row: Option<(String, String, Option<i64>, Option<i64>)> = tx
        .query_row(
            "SELECT status, due_date, awarded_coins, awarded_xp FROM occurrences WHERE id = ?1",
            [occ_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((status, due, coins, xp)) = row else { return rule("Task not found.") };
    if status != "completed" {
        return rule("This task isn't done.");
    }
    if due != ds(today) {
        return rule("That day is already closed, so it can't be undone.");
    }
    let (coins, xp) = (coins.unwrap_or(0), xp.unwrap_or(0));
    if balance(&tx)? < coins {
        return rule("You've already spent these coins, so this can't be undone.");
    }
    tx.execute(
        "UPDATE occurrences SET status='pending', completed_at=NULL, awarded_coins=NULL, awarded_xp=NULL, completed_seq=NULL WHERE id=?1",
        [occ_id],
    )?;
    tx.execute(
        "INSERT INTO ledger (op_id, kind, coins, xp, occurrence_id, created_at) VALUES (?1,'undo',?2,?3,?4,?5)",
        params![op_id, -coins, -xp, occ_id, now_utc()],
    )?;
    let b = balance(&tx)?;
    tx.commit()?;
    Ok(b)
}

/// "I'm not doing this today." Counts as not done for the streak.
pub fn skip_occurrence(conn: &mut Connection, today: NaiveDate, occ_id: i64) -> Result<()> {
    sync_day(conn, today)?;
    let n = conn.execute(
        "UPDATE occurrences SET status='skipped' WHERE id=?1 AND status='pending' AND due_date=?2",
        params![occ_id, ds(today)],
    )?;
    if n == 0 {
        return rule("Only today's unfinished tasks can be skipped.");
    }
    Ok(())
}

/// Archive a task: no new occurrences, and its open occurrence is cancelled.
/// History (completed occurrences, ledger) is kept.
pub fn remove_task(conn: &mut Connection, today: NaiveDate, task_id: i64) -> Result<()> {
    sync_day(conn, today)?;
    let tx = conn.transaction()?;
    let n = tx.execute("UPDATE tasks SET status='archived' WHERE id=?1", [task_id])?;
    if n == 0 {
        return rule("Task not found.");
    }
    tx.execute("UPDATE occurrences SET status='cancelled' WHERE task_id=?1 AND status='pending'", [task_id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, DATE_FMT).unwrap()
    }
    const DAY0: &str = "2026-10-19";

    fn add(conn: &mut Connection, today: NaiveDate, title: &str, size: Size, schedule: Schedule) -> i64 {
        create_task(conn, today, NewTask { title: title.into(), details: String::new(), size, schedule, start_date: None, end_date: None }).unwrap()
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn complete_awards_once_per_op_id() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        add(&mut c, t, "Read", Size::Small, Schedule::Daily);
        let occ = get_state(&mut c, t).unwrap().cards[0].occurrence_id;
        let r1 = complete_occurrence(&mut c, t, "op-1", occ).unwrap();
        let r2 = complete_occurrence(&mut c, t, "op-1", occ).unwrap();
        assert_eq!((r1.coins, r1.xp, r1.balance), (10, 10, 10));
        assert_eq!(r2.balance, 10);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM ledger"), 1);
        // A second, different click on a done task is refused.
        assert!(complete_occurrence(&mut c, t, "op-2", occ).is_err());
        assert_eq!(count(&c, "SELECT COUNT(*) FROM ledger"), 1);
    }

    #[test]
    fn completed_tasks_sort_last_newest_at_bottom() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        for n in ["A", "B", "C"] {
            add(&mut c, t, n, Size::Small, Schedule::Once);
        }
        let ids: Vec<i64> = get_state(&mut c, t).unwrap().cards.iter().map(|x| x.occurrence_id).collect();
        complete_occurrence(&mut c, t, "1", ids[1]).unwrap();
        complete_occurrence(&mut c, t, "2", ids[0]).unwrap();
        let order: Vec<String> = get_state(&mut c, t).unwrap().cards.into_iter().map(|x| x.title).collect();
        assert_eq!(order, ["C", "B", "A"]);
    }

    #[test]
    fn missed_days_roll_over_once_per_day_with_penalty() {
        let mut c = db::open_in_memory().unwrap();
        add(&mut c, d(DAY0), "Gym", Size::Small, Schedule::Daily);
        let s = get_state(&mut c, d("2026-10-24")).unwrap(); // 5 days later
        assert_eq!(s.cards.len(), 1, "no duplicate open occurrences");
        assert_eq!(s.cards[0].rollover_count, 5);
        assert_eq!(s.cards[0].coins, 3); // round(10 × 0.8^5) = round(3.28)
        assert_eq!(count(&c, "SELECT COUNT(*) FROM days"), 5);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM days WHERE result='broken'"), 5);
        // Running again changes nothing.
        get_state(&mut c, d("2026-10-24")).unwrap();
        assert_eq!(count(&c, "SELECT COUNT(*) FROM days"), 5);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM occurrences"), 1);
    }

    #[test]
    fn completing_carried_over_task_does_not_spawn_another_today() {
        let mut c = db::open_in_memory().unwrap();
        add(&mut c, d(DAY0), "Gym", Size::Small, Schedule::Daily);
        let t1 = d("2026-10-20");
        let occ = get_state(&mut c, t1).unwrap().cards[0].occurrence_id;
        let r = complete_occurrence(&mut c, t1, "x", occ).unwrap();
        assert_eq!(r.coins, 8);
        let s = get_state(&mut c, t1).unwrap();
        assert_eq!(s.cards.len(), 1);
        assert_eq!(s.cards[0].status, "completed");
        // Next day a fresh one appears.
        let s = get_state(&mut c, d("2026-10-21")).unwrap();
        assert_eq!(s.cards.len(), 1);
        assert_eq!(s.cards[0].rollover_count, 0);
    }

    #[test]
    fn clock_moving_back_changes_nothing() {
        let mut c = db::open_in_memory().unwrap();
        add(&mut c, d(DAY0), "Gym", Size::Small, Schedule::Daily);
        get_state(&mut c, d("2026-10-24")).unwrap();
        let occ_before = count(&c, "SELECT COUNT(*) FROM occurrences");
        let days_before = count(&c, "SELECT COUNT(*) FROM days");
        let s = get_state(&mut c, d("2026-10-21")).unwrap();
        assert!(s.cards.is_empty());
        assert_eq!(count(&c, "SELECT COUNT(*) FROM occurrences"), occ_before);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM days"), days_before);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM ledger"), 0);
    }

    #[test]
    fn streak_and_neutral_days() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        add(&mut c, t, "A", Size::Small, Schedule::Once);
        add(&mut c, t, "B", Size::Small, Schedule::Once);
        let occ = get_state(&mut c, t).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, t, "a", occ).unwrap();
        // 1 of 2 = 50% → qualifies.
        assert_eq!(get_state(&mut c, d("2026-10-20")).unwrap().streak, 1);
        // B rolled to 10-20 and wasn't done → breaks.
        assert_eq!(get_state(&mut c, d("2026-10-21")).unwrap().streak, 0);
    }

    #[test]
    fn empty_day_is_neutral() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        add(&mut c, t, "A", Size::Small, Schedule::Once);
        let occ = get_state(&mut c, t).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, t, "a", occ).unwrap();
        let s = get_state(&mut c, d("2026-10-23")).unwrap();
        assert_eq!(s.streak, 1);
        assert_eq!(count(&c, "SELECT COUNT(*) FROM days WHERE result='neutral'"), 3);
    }

    #[test]
    fn skipped_counts_as_not_done_cancelled_is_excluded() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        let a = add(&mut c, t, "A", Size::Small, Schedule::Once);
        add(&mut c, t, "B", Size::Small, Schedule::Once);
        add(&mut c, t, "C", Size::Small, Schedule::Once);
        let cards = get_state(&mut c, t).unwrap().cards;
        complete_occurrence(&mut c, t, "1", cards[0].occurrence_id).unwrap(); // A done
        skip_occurrence(&mut c, t, cards[1].occurrence_id).unwrap(); // B skipped
        remove_task(&mut c, t, cards[2].task_id).unwrap(); // C cancelled
        let _ = a;
        get_state(&mut c, d("2026-10-20")).unwrap();
        let (elig, done): (i64, i64) =
            c.query_row("SELECT eligible, completed FROM days WHERE date=?1", [DAY0], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((elig, done), (2, 1));
    }

    #[test]
    fn uncomplete_reverses_and_refuses_if_spent() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        add(&mut c, t, "A", Size::Medium, Schedule::Once);
        let occ = get_state(&mut c, t).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, t, "c1", occ).unwrap();
        assert_eq!(uncomplete_occurrence(&mut c, t, "u1", occ).unwrap(), 0);
        let s = get_state(&mut c, t).unwrap();
        assert_eq!((s.balance, s.xp, s.cards[0].status.as_str()), (0, 0, "pending"));
        // Complete again, spend the coins, then undo is refused and nothing changes.
        complete_occurrence(&mut c, t, "c2", occ).unwrap();
        c.execute("INSERT INTO ledger (op_id, kind, coins, xp, created_at) VALUES ('spend','redeem',-15,0,'x')", []).unwrap();
        assert!(uncomplete_occurrence(&mut c, t, "u2", occ).is_err());
        assert_eq!(get_state(&mut c, t).unwrap().balance, 5);
    }

    #[test]
    fn cannot_undo_after_day_closes() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        add(&mut c, t, "A", Size::Small, Schedule::Once);
        let occ = get_state(&mut c, t).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, t, "c1", occ).unwrap();
        assert!(uncomplete_occurrence(&mut c, d("2026-10-20"), "u1", occ).is_err());
    }

    #[test]
    fn title_validation() {
        let mut c = db::open_in_memory().unwrap();
        let t = d(DAY0);
        let bad = |title: String| NewTask { title, details: String::new(), size: Size::Small, schedule: Schedule::Once, start_date: None, end_date: None };
        assert!(create_task(&mut c, t, bad("   ".into())).is_err());
        assert!(create_task(&mut c, t, bad("x".repeat(121))).is_err());
        assert!(create_task(&mut c, t, bad("x".repeat(120))).is_ok());
    }

    #[test]
    fn migrations_on_file_survive_reopen() {
        let dir = std::env::temp_dir().join(format!("tw-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.db");
        let _ = std::fs::remove_file(&path);
        {
            let mut c = db::open(&path).unwrap();
            add(&mut c, d(DAY0), "Persist me", Size::Small, Schedule::Once);
        }
        let mut c = db::open(&path).unwrap();
        let v: usize = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, db::schema_version());
        assert_eq!(get_state(&mut c, d(DAY0)).unwrap().cards[0].title, "Persist me");
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
