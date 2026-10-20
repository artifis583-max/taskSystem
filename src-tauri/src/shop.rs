//! Phase 5: the reward shop and streak revives.
//! Same rules as service.rs: every write is one transaction, spends never make the balance negative.

use crate::config;
use crate::service::{balance, check_op_id, current_streak, ds, now_utc, op_seen, rule, sync_day, Result};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ---------- revives ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviveInfo {
    /// This week's free revive hasn't been used yet.
    pub free_available: bool,
    /// Bought revives not yet used.
    pub bought: i64,
    pub price: i64,
    /// Set when yesterday broke a streak that can still be revived today.
    pub broken_date: Option<String>,
    /// The streak a revive would bring back.
    pub streak_to_restore: i64,
}

/// Monday of the week containing `d` (local calendar).
pub fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

/// A break can be revived only on the day right after it, only if it is the latest
/// closed day, only if there was a streak to lose, and only once.
fn revivable(c: &Connection, today: NaiveDate) -> Result<Option<(NaiveDate, i64)>> {
    let Some(yesterday) = today.pred_opt() else { return Ok(None) };
    let last: Option<(String, String)> = c
        .query_row("SELECT date, result FROM days ORDER BY date DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((date, result)) = last else { return Ok(None) };
    if date != ds(yesterday) || result != "broken" {
        return Ok(None);
    }
    let used: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM revives WHERE used_for_date = ?1)", [&date], |r| r.get(0))?;
    if used {
        return Ok(None);
    }
    let before: i64 = c
        .query_row("SELECT streak_after FROM days WHERE date < ?1 ORDER BY date DESC LIMIT 1", [&date], |r| r.get(0))
        .optional()?
        .unwrap_or(0);
    Ok((before > 0).then_some((yesterday, before)))
}

fn free_available(c: &Connection, today: NaiveDate) -> Result<bool> {
    let used: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM revives WHERE source = 'free' AND week_of = ?1)",
        [ds(week_start(today))],
        |r| r.get(0),
    )?;
    Ok(!used)
}

fn bought_unused(c: &Connection) -> Result<i64> {
    Ok(c.query_row("SELECT COUNT(*) FROM revives WHERE source = 'bought' AND used_at IS NULL", [], |r| r.get(0))?)
}

pub fn revive_info(c: &Connection, today: NaiveDate) -> Result<ReviveInfo> {
    let rev = revivable(c, today)?;
    Ok(ReviveInfo {
        free_available: free_available(c, today)?,
        bought: bought_unused(c)?,
        price: config::REVIVE_PRICE,
        broken_date: rev.map(|(d, _)| ds(d)),
        streak_to_restore: rev.map(|(_, s)| s).unwrap_or(0),
    })
}

/// Spend coins on one revive. Returns the new balance.
pub fn buy_revive(conn: &mut Connection, today: NaiveDate, op_id: &str) -> Result<i64> {
    check_op_id(op_id)?;
    sync_day(conn, today)?;
    let tx = conn.transaction()?;
    if op_seen(&tx, op_id)? {
        return balance(&tx);
    }
    if balance(&tx)? < config::REVIVE_PRICE {
        return rule(&format!("A revive costs {} coins. You don't have enough yet.", config::REVIVE_PRICE));
    }
    let now = now_utc();
    tx.execute(
        "INSERT INTO ledger (op_id, kind, coins, xp, created_at) VALUES (?1, 'revive_buy', ?2, 0, ?3)",
        params![op_id, -config::REVIVE_PRICE, now],
    )?;
    tx.execute("INSERT INTO revives (source) VALUES ('bought')", [])?;
    let b = balance(&tx)?;
    tx.commit()?;
    Ok(b)
}

/// Bring back yesterday's broken streak. Uses the free weekly revive first, then a bought one.
/// Returns the restored streak.
pub fn use_revive(conn: &mut Connection, today: NaiveDate) -> Result<i64> {
    sync_day(conn, today)?;
    let tx = conn.transaction()?;
    let Some((date, streak)) = revivable(&tx, today)? else {
        return rule("There's no broken streak to revive right now.");
    };
    let now = now_utc();
    if free_available(&tx, today)? {
        tx.execute(
            "INSERT INTO revives (source, week_of, used_for_date, used_at) VALUES ('free', ?1, ?2, ?3)",
            params![ds(week_start(today)), ds(date), now],
        )?;
    } else if bought_unused(&tx)? > 0 {
        tx.execute(
            "UPDATE revives SET used_for_date = ?1, used_at = ?2
             WHERE id = (SELECT id FROM revives WHERE source = 'bought' AND used_at IS NULL ORDER BY id LIMIT 1)",
            params![ds(date), now],
        )?;
    } else {
        return rule(&format!("You have no revives left. Buy one for {} coins.", config::REVIVE_PRICE));
    }
    tx.execute("UPDATE days SET result = 'revived', streak_after = ?2 WHERE date = ?1", params![ds(date), streak])?;
    debug_assert_eq!(current_streak(&tx)?, streak);
    tx.commit()?;
    Ok(streak)
}

// ---------- reward shop ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardInput {
    pub name: String,
    #[serde(default)]
    pub note: String,
    pub price: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    pub id: i64,
    pub name: String,
    pub note: String,
    pub price: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Redemption {
    pub id: i64,
    pub name: String,
    pub price: i64,
    /// UTC, RFC 3339.
    pub created_at: String,
}

fn validate(r: &RewardInput) -> Result<(String, String)> {
    let name = r.name.trim().to_string();
    let len = name.chars().count();
    if len == 0 || len > config::REWARD_NAME_MAX {
        return rule(&format!("Reward name must be 1–{} characters.", config::REWARD_NAME_MAX));
    }
    let note = r.note.trim().to_string();
    if note.chars().count() > config::REWARD_NOTE_MAX {
        return rule(&format!("Note must be {} characters or fewer.", config::REWARD_NOTE_MAX));
    }
    if r.price < 1 || r.price > config::REWARD_PRICE_MAX {
        return rule("Price must be at least 1 coin.");
    }
    Ok((name, note))
}

pub fn list_rewards(c: &Connection) -> Result<Vec<Reward>> {
    let mut stmt = c.prepare("SELECT id, name, note, price FROM rewards WHERE archived = 0 ORDER BY price, id")?;
    let rows = stmt
        .query_map([], |r| Ok(Reward { id: r.get(0)?, name: r.get(1)?, note: r.get(2)?, price: r.get(3)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn recent_redemptions(c: &Connection, limit: i64) -> Result<Vec<Redemption>> {
    let mut stmt = c.prepare(
        "SELECT d.id, r.name, d.price_paid, d.created_at FROM redemptions d JOIN rewards r ON r.id = d.reward_id
         ORDER BY d.id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit], |r| Ok(Redemption { id: r.get(0)?, name: r.get(1)?, price: r.get(2)?, created_at: r.get(3)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn create_reward(conn: &mut Connection, input: RewardInput) -> Result<i64> {
    let (name, note) = validate(&input)?;
    conn.execute("INSERT INTO rewards (name, note, price) VALUES (?1, ?2, ?3)", params![name, note, input.price])?;
    Ok(conn.last_insert_rowid())
}

/// Changing a price only affects future redemptions; history keeps the price paid.
pub fn update_reward(conn: &mut Connection, id: i64, input: RewardInput) -> Result<()> {
    let (name, note) = validate(&input)?;
    let n = conn.execute(
        "UPDATE rewards SET name = ?2, note = ?3, price = ?4 WHERE id = ?1 AND archived = 0",
        params![id, name, note, input.price],
    )?;
    if n == 0 {
        return rule("Reward not found.");
    }
    Ok(())
}

/// Hide a reward from the shop. Its redemption history is kept.
pub fn archive_reward(conn: &mut Connection, id: i64) -> Result<()> {
    let n = conn.execute("UPDATE rewards SET archived = 1 WHERE id = ?1", [id])?;
    if n == 0 {
        return rule("Reward not found.");
    }
    Ok(())
}

/// Spend coins on a reward. Returns the new balance.
pub fn redeem_reward(conn: &mut Connection, today: NaiveDate, op_id: &str, id: i64) -> Result<i64> {
    check_op_id(op_id)?;
    sync_day(conn, today)?;
    let tx = conn.transaction()?;
    if op_seen(&tx, op_id)? {
        return balance(&tx);
    }
    let price: Option<i64> = tx
        .query_row("SELECT price FROM rewards WHERE id = ?1 AND archived = 0", [id], |r| r.get(0))
        .optional()?;
    let Some(price) = price else { return rule("Reward not found.") };
    let bal = balance(&tx)?;
    if bal < price {
        return rule(&format!("You need {} more coins for this.", price - bal));
    }
    let now = now_utc();
    tx.execute(
        "INSERT INTO ledger (op_id, kind, coins, xp, created_at) VALUES (?1, 'redeem', ?2, 0, ?3)",
        params![op_id, -price, now],
    )?;
    let ledger_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO redemptions (reward_id, price_paid, ledger_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, price, ledger_id, now],
    )?;
    let b = balance(&tx)?;
    tx.commit()?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::domain::{Schedule, Size};
    use crate::service::{complete_occurrence, create_task, get_state, NewTask};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn give_coins(c: &Connection, n: i64) {
        c.execute(
            "INSERT INTO ledger (op_id, kind, coins, xp, created_at) VALUES (?1, 'correction', ?2, 0, 'x')",
            params![format!("gift-{n}-{}", rand_suffix()), n],
        )
        .unwrap();
    }
    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    fn task(c: &mut Connection, today: NaiveDate, title: &str) {
        create_task(c, today, NewTask { title: title.into(), details: String::new(), size: Size::Small, schedule: Schedule::Once, start_date: None })
            .unwrap();
    }

    /// Mon 10-19 qualifies (streak 1), Tue 10-20 breaks. Today = Wed 10-21.
    fn broken_yesterday() -> Connection {
        let mut c = db::open_in_memory().unwrap();
        let mon = d("2026-10-19");
        task(&mut c, mon, "A");
        let occ = get_state(&mut c, mon).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, mon, "a", occ).unwrap();
        task(&mut c, d("2026-10-20"), "B");
        c
    }

    #[test]
    fn week_starts_monday() {
        assert_eq!(week_start(d("2026-10-25")), d("2026-10-19")); // Sunday
        assert_eq!(week_start(d("2026-10-26")), d("2026-10-26")); // Monday
    }

    #[test]
    fn revive_restores_streak_the_day_after_a_break() {
        let mut c = broken_yesterday();
        let wed = d("2026-10-21");
        let s = get_state(&mut c, wed).unwrap();
        assert_eq!(s.streak, 0);
        assert_eq!(s.revives.broken_date.as_deref(), Some("2026-10-20"));
        assert_eq!(s.revives.streak_to_restore, 1);
        assert_eq!(use_revive(&mut c, wed).unwrap(), 1);
        let s = get_state(&mut c, wed).unwrap();
        assert_eq!(s.streak, 1);
        assert!(!s.revives.free_available, "free revive was used");
        assert!(s.revives.broken_date.is_none());
        // Same break can't be revived twice, even with a bought revive.
        give_coins(&c, 100);
        buy_revive(&mut c, wed, "buy1").unwrap();
        assert!(use_revive(&mut c, wed).is_err());
        assert_eq!(bought_unused(&c).unwrap(), 1);
    }

    #[test]
    fn revive_window_closes_after_one_day() {
        let mut c = broken_yesterday();
        let s = get_state(&mut c, d("2026-10-22")).unwrap();
        assert!(s.revives.broken_date.is_none());
        assert!(use_revive(&mut c, d("2026-10-22")).is_err());
    }

    #[test]
    fn bought_revive_used_when_free_is_gone() {
        let mut c = broken_yesterday();
        let wed = d("2026-10-21");
        // Pretend this week's free revive was already used.
        c.execute("INSERT INTO revives (source, week_of, used_for_date, used_at) VALUES ('free','2026-10-19','2026-10-01','x')", []).unwrap();
        assert!(use_revive(&mut c, wed).is_err(), "no revives at all");
        give_coins(&c, 150);
        assert_eq!(buy_revive(&mut c, wed, "buy").unwrap(), 150 + 10 - 100);
        assert_eq!(use_revive(&mut c, wed).unwrap(), 1);
        assert_eq!(bought_unused(&c).unwrap(), 0);
    }

    #[test]
    fn free_revive_refills_on_monday() {
        let mut c = db::open_in_memory().unwrap();
        c.execute("INSERT INTO revives (source, week_of, used_for_date, used_at) VALUES ('free','2026-10-19','2026-10-20','x')", []).unwrap();
        assert!(!revive_info(&c, d("2026-10-25")).unwrap().free_available); // Sunday
        assert!(revive_info(&c, d("2026-10-26")).unwrap().free_available); // Monday
        let _ = &mut c;
    }

    #[test]
    fn buying_needs_enough_coins_and_is_idempotent() {
        let mut c = db::open_in_memory().unwrap();
        let t = d("2026-10-19");
        give_coins(&c, 99);
        assert!(buy_revive(&mut c, t, "b1").is_err());
        give_coins(&c, 101);
        assert_eq!(buy_revive(&mut c, t, "b2").unwrap(), 100);
        assert_eq!(buy_revive(&mut c, t, "b2").unwrap(), 100);
        assert_eq!(bought_unused(&c).unwrap(), 1);
    }

    #[test]
    fn redeem_rules() {
        let mut c = db::open_in_memory().unwrap();
        let t = d("2026-10-19");
        let movie = create_reward(&mut c, RewardInput { name: "Movie night".into(), note: String::new(), price: 150 }).unwrap();
        give_coins(&c, 100);
        assert!(redeem_reward(&mut c, t, "r1", movie).is_err());
        assert_eq!(balance(&c).unwrap(), 100);
        give_coins(&c, 60);
        assert_eq!(redeem_reward(&mut c, t, "r2", movie).unwrap(), 10);
        assert_eq!(redeem_reward(&mut c, t, "r2", movie).unwrap(), 10, "same click twice");
        assert_eq!(recent_redemptions(&c, 5).unwrap().len(), 1);
        // Price change doesn't rewrite history; archive hides it but keeps history.
        update_reward(&mut c, movie, RewardInput { name: "Movie night".into(), note: String::new(), price: 200 }).unwrap();
        assert_eq!(recent_redemptions(&c, 5).unwrap()[0].price, 150);
        archive_reward(&mut c, movie).unwrap();
        assert!(list_rewards(&c).unwrap().is_empty());
        assert_eq!(recent_redemptions(&c, 5).unwrap().len(), 1);
        give_coins(&c, 500);
        assert!(redeem_reward(&mut c, t, "r3", movie).is_err(), "archived");
    }

    #[test]
    fn reward_validation() {
        let mut c = db::open_in_memory().unwrap();
        let bad = |name: &str, price| RewardInput { name: name.into(), note: String::new(), price };
        assert!(create_reward(&mut c, bad("  ", 10)).is_err());
        assert!(create_reward(&mut c, bad("Snack", 0)).is_err());
        assert!(create_reward(&mut c, bad(&"x".repeat(61), 10)).is_err());
        assert!(create_reward(&mut c, bad("Snack", 25)).is_ok());
    }
}
