//! Task management, history and stats for the Upcoming tab and the dashboard.

use crate::config;
use crate::domain::{self, Schedule, Size};
use crate::service::{
    current_streak, ds, pd, rule, sync_day, total_xp, validate_task, NewTask, Result,
};
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

// ---------- tasks ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub id: i64,
    pub title: String,
    pub details: String,
    pub size: Size,
    pub schedule: Schedule,
    pub start_date: String,
    pub status: String,
}

/// Active and paused tasks, in list order. Archived tasks are hidden.
pub fn list_tasks(c: &Connection) -> Result<Vec<TaskInfo>> {
    let mut stmt = c.prepare(
        "SELECT id, title, details, size, schedule_json, start_date, status FROM tasks
         WHERE status != 'archived' ORDER BY status = 'paused', sort_order, id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let size: String = r.get(3)?;
            let sched: String = r.get(4)?;
            Ok(TaskInfo {
                id: r.get(0)?,
                title: r.get(1)?,
                details: r.get(2)?,
                size: Size::parse(&size).unwrap_or(Size::Small),
                schedule: serde_json::from_str(&sched).unwrap_or(Schedule::Once),
                start_date: r.get(5)?,
                status: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Edit a task. Changes apply to future days; completed history and awarded coins stay as they were.
pub fn update_task(conn: &mut Connection, today: NaiveDate, id: i64, t: NewTask) -> Result<()> {
    let old: Option<String> = conn
        .query_row("SELECT start_date FROM tasks WHERE id = ?1 AND status != 'archived'", [id], |r| r.get(0))
        .optional()?;
    let Some(old) = old else { return rule("Task not found.") };
    let v = validate_task(&t, today, Some(pd(&old)?))?;
    conn.execute(
        "UPDATE tasks SET title = ?2, details = ?3, size = ?4, schedule_json = ?5, start_date = ?6 WHERE id = ?1",
        params![id, v.title, v.details, t.size.as_str(), v.schedule_json, ds(v.start)],
    )?;
    sync_day(conn, today)?;
    Ok(())
}

/// Pause or resume. Paused tasks get no new days; an open one stays until done, skipped or removed.
pub fn set_task_status(conn: &mut Connection, today: NaiveDate, id: i64, status: &str) -> Result<()> {
    if status != "active" && status != "paused" {
        return rule("Status must be active or paused.");
    }
    let n = conn.execute("UPDATE tasks SET status = ?2 WHERE id = ?1 AND status != 'archived'", params![id, status])?;
    if n == 0 {
        return rule("Task not found.");
    }
    sync_day(conn, today)?;
    Ok(())
}

// ---------- upcoming ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingItem {
    pub task_id: i64,
    pub title: String,
    pub details: String,
    pub size: Size,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingDay {
    pub date: String,
    pub items: Vec<UpcomingItem>,
}

/// What's scheduled on each of the next `days` days (after today), from active tasks' schedules.
/// Unfinished tasks from today will also roll into tomorrow; that isn't shown here.
pub fn upcoming(c: &Connection, today: NaiveDate, days: i64) -> Result<Vec<UpcomingDay>> {
    let tasks = list_tasks(c)?;
    let mut out = Vec::new();
    for i in 1..=days {
        let d = today + Duration::days(i);
        let mut items = Vec::new();
        for t in tasks.iter().filter(|t| t.status == "active") {
            if t.schedule.occurs_on(pd(&t.start_date)?, None, d) {
                items.push(UpcomingItem { task_id: t.id, title: t.title.clone(), details: t.details.clone(), size: t.size });
            }
        }
        out.push(UpcomingDay { date: ds(d), items });
    }
    Ok(out)
}

// ---------- history and stats ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayInfo {
    pub date: String,
    pub eligible: i64,
    pub completed: i64,
    pub result: String,
    pub streak_after: i64,
}

/// Closed days, newest first.
pub fn history(c: &Connection, limit: i64) -> Result<Vec<DayInfo>> {
    let mut stmt =
        c.prepare("SELECT date, eligible, completed, result, streak_after FROM days ORDER BY date DESC LIMIT ?1")?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(DayInfo { date: r.get(0)?, eligible: r.get(1)?, completed: r.get(2)?, result: r.get(3)?, streak_after: r.get(4)? })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelRow {
    pub level: i64,
    pub xp: i64,
    pub streak: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub level: i64,
    pub xp: i64,
    pub streak: i64,
    pub best_streak: i64,
    pub coins_earned: i64,
    pub coins_spent: i64,
    pub tasks_completed: i64,
    /// Completed ÷ eligible over the last 30 closed days, 0–100. None if there were no tasks.
    pub completion_30d: Option<i64>,
    pub levels: Vec<LevelRow>,
}

pub fn stats(conn: &mut Connection, today: NaiveDate) -> Result<Stats> {
    sync_day(conn, today)?;
    let c = &*conn;
    let xp = total_xp(c)?;
    let streak = current_streak(c)?;
    let best: i64 = c.query_row("SELECT COALESCE(MAX(streak_after),0) FROM days", [], |r| r.get(0))?;
    let earned: i64 = c.query_row("SELECT COALESCE(SUM(coins),0) FROM ledger WHERE kind IN ('task','undo','correction') ", [], |r| r.get(0))?;
    let spent: i64 = c.query_row("SELECT COALESCE(-SUM(coins),0) FROM ledger WHERE kind IN ('redeem','revive_buy')", [], |r| r.get(0))?;
    let done: i64 = c.query_row("SELECT COUNT(*) FROM occurrences WHERE status = 'completed'", [], |r| r.get(0))?;
    let since = ds(today - Duration::days(30));
    let (elig, comp): (i64, i64) = c.query_row(
        "SELECT COALESCE(SUM(eligible),0), COALESCE(SUM(completed),0) FROM days WHERE date >= ?1",
        params![since],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(Stats {
        level: domain::level_for(xp, streak),
        xp,
        streak,
        best_streak: best,
        coins_earned: earned,
        coins_spent: spent,
        tasks_completed: done,
        completion_30d: (elig > 0).then(|| (comp * 100 + elig / 2) / elig),
        levels: config::LEVELS.iter().map(|&(level, xp, streak)| LevelRow { level, xp, streak }).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::service::{complete_occurrence, create_task, get_state};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn nt(title: &str, schedule: Schedule, start: Option<&str>) -> NewTask {
        NewTask { title: title.into(), details: String::new(), size: Size::Small, schedule, start_date: start.map(String::from) }
    }

    #[test]
    fn upcoming_follows_schedules_and_future_starts() {
        let mut c = db::open_in_memory().unwrap();
        let mon = d("2026-10-19");
        create_task(&mut c, mon, nt("Gym", Schedule::Weekdays { days: vec![0, 2, 4] }, None)).unwrap();
        create_task(&mut c, mon, nt("Dentist", Schedule::Once, Some("2026-10-22"))).unwrap();
        let up = upcoming(&c, mon, 7).unwrap();
        assert_eq!(up.len(), 7);
        let titles = |date: &str| up.iter().find(|x| x.date == date).unwrap().items.iter().map(|i| i.title.clone()).collect::<Vec<_>>();
        assert_eq!(titles("2026-10-20"), Vec::<String>::new()); // Tue
        assert_eq!(titles("2026-10-21"), ["Gym"]); // Wed
        assert_eq!(titles("2026-10-22"), ["Dentist"]); // Thu
        // The future one-time task isn't on today's list.
        assert_eq!(get_state(&mut c, mon).unwrap().cards.len(), 1);
    }

    #[test]
    fn future_start_task_appears_on_its_day() {
        let mut c = db::open_in_memory().unwrap();
        create_task(&mut c, d("2026-10-19"), nt("Dentist", Schedule::Once, Some("2026-10-21"))).unwrap();
        let s = get_state(&mut c, d("2026-10-21")).unwrap();
        assert_eq!(s.cards.len(), 1);
        assert_eq!(s.cards[0].rollover_count, 0);
    }

    #[test]
    fn past_start_rejected_but_existing_start_kept_on_edit() {
        let mut c = db::open_in_memory().unwrap();
        let mon = d("2026-10-19");
        assert!(create_task(&mut c, mon, nt("X", Schedule::Daily, Some("2026-10-18"))).is_err());
        let id = create_task(&mut c, mon, nt("Read", Schedule::Daily, None)).unwrap();
        // A week later, editing without moving the start is fine; moving it into the past isn't.
        let later = d("2026-10-26");
        update_task(&mut c, later, id, nt("Read 30 pages", Schedule::Daily, Some("2026-10-19"))).unwrap();
        assert!(update_task(&mut c, later, id, nt("Read", Schedule::Daily, Some("2026-10-20"))).is_err());
        assert_eq!(list_tasks(&c).unwrap()[0].title, "Read 30 pages");
    }

    #[test]
    fn edit_keeps_awarded_history() {
        let mut c = db::open_in_memory().unwrap();
        let mon = d("2026-10-19");
        let id = create_task(&mut c, mon, nt("Read", Schedule::Daily, None)).unwrap();
        let occ = get_state(&mut c, mon).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, mon, "x", occ).unwrap();
        let mut big = nt("Read", Schedule::Daily, None);
        big.size = Size::Large;
        update_task(&mut c, mon, id, big).unwrap();
        let s = get_state(&mut c, mon).unwrap();
        assert_eq!((s.cards[0].coins, s.balance), (10, 10));
        assert_eq!(get_state(&mut c, d("2026-10-20")).unwrap().cards[0].coins, 30);
    }

    #[test]
    fn paused_tasks_stop_generating() {
        let mut c = db::open_in_memory().unwrap();
        let mon = d("2026-10-19");
        let id = create_task(&mut c, mon, nt("Gym", Schedule::Daily, None)).unwrap();
        let occ = get_state(&mut c, mon).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, mon, "x", occ).unwrap();
        set_task_status(&mut c, mon, id, "paused").unwrap();
        assert!(get_state(&mut c, d("2026-10-20")).unwrap().cards.is_empty());
        assert!(upcoming(&c, d("2026-10-20"), 7).unwrap().iter().all(|x| x.items.is_empty()));
        set_task_status(&mut c, d("2026-10-20"), id, "active").unwrap();
        assert_eq!(get_state(&mut c, d("2026-10-20")).unwrap().cards.len(), 1);
        assert!(set_task_status(&mut c, mon, id, "archived").is_err());
    }

    #[test]
    fn stats_and_history() {
        let mut c = db::open_in_memory().unwrap();
        let mon = d("2026-10-19");
        create_task(&mut c, mon, nt("A", Schedule::Daily, None)).unwrap();
        let occ = get_state(&mut c, mon).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, mon, "x", occ).unwrap();
        let st = stats(&mut c, d("2026-10-21")).unwrap(); // Mon qualified, Tue broken
        assert_eq!((st.streak, st.best_streak, st.tasks_completed, st.coins_earned), (0, 1, 1, 10));
        assert_eq!(st.completion_30d, Some(50));
        assert_eq!(st.levels.len(), 7);
        let h = history(&c, 10).unwrap();
        assert_eq!(h.iter().map(|x| x.result.as_str()).collect::<Vec<_>>(), ["broken", "qualified"]);
    }
}
