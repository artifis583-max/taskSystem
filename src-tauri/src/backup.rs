//! Backups: consistent snapshots of the database, automatic rolling backups,
//! and validated restore. Restoring never touches live data until the file has passed every check.

use crate::db;
use crate::service::{rule, AppError, Result};
use chrono::{DateTime, Local, Utc};
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How many automatic backups to keep.
pub const KEEP_AUTO: usize = 7;
const PREFIX: &str = "taskwidget-";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub name: String,
    pub size: u64,
    /// UTC, RFC 3339.
    pub created_at: String,
    /// "auto", "manual", "before-restore", "pre-upgrade-vN"
    pub kind: String,
}

fn io(e: std::io::Error) -> AppError {
    AppError::Rule(format!("File error: {e}"))
}

/// Write a consistent copy of the live database to `dir`, named with `kind` and the time.
pub fn snapshot(conn: &Connection, dir: &Path, kind: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).map_err(io)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let mut path = dir.join(format!("{PREFIX}{stamp}-{kind}.db"));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("{PREFIX}{stamp}-{kind}-{n}.db"));
    }
    write_copy(conn, &path)?;
    if kind == "auto" {
        prune(dir)?;
    }
    Ok(path)
}

/// Copy the database to `path` (replacing it). Written to a temp file first, then renamed,
/// so a crash never leaves a half-written backup under the final name.
pub fn write_copy(conn: &Connection, path: &Path) -> Result<()> {
    let tmp = path.with_extension("db.partial");
    let _ = std::fs::remove_file(&tmp);
    conn.execute("VACUUM INTO ?1", [tmp.to_string_lossy()])?;
    if path.exists() {
        std::fs::remove_file(path).map_err(io)?;
    }
    std::fs::rename(&tmp, path).map_err(io)?;
    Ok(())
}

/// Keep only the newest KEEP_AUTO automatic backups. Other kinds are kept.
fn prune(dir: &Path) -> Result<()> {
    let mut autos: Vec<_> = list(dir)?.into_iter().filter(|b| b.kind == "auto").collect();
    autos.sort_by(|a, b| b.name.cmp(&a.name));
    for old in autos.into_iter().skip(KEEP_AUTO) {
        let _ = std::fs::remove_file(dir.join(old.name));
    }
    Ok(())
}

/// Backups in `dir`, newest first.
pub fn list(dir: &Path) -> Result<Vec<BackupFile>> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(vec![]) };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix(PREFIX).and_then(|r| r.strip_suffix(".db")) else { continue };
        // rest = YYYYMMDD-HHMMSS-kind[-n]
        let kind = rest.get(16..).unwrap_or("manual");
        // Drop a "-2" style duplicate counter, but keep names like "pre-upgrade-v1".
        let kind = match kind.rsplit_once('-') {
            Some((k, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => k.to_string(),
            _ => kind.to_string(),
        };
        let meta = e.metadata().map_err(io)?;
        let created: DateTime<Utc> = meta.modified().unwrap_or(SystemTime::now()).into();
        out.push(BackupFile { name, size: meta.len(), created_at: created.to_rfc3339(), kind });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Make the daily automatic backup if the newest one is older than ~a day.
pub fn auto_if_due(conn: &Connection, dir: &Path) -> Result<Option<PathBuf>> {
    let newest = list(dir)?
        .into_iter()
        .filter(|b| b.kind == "auto")
        .filter_map(|b| std::fs::metadata(dir.join(&b.name)).ok()?.modified().ok())
        .max();
    let due = newest.map_or(true, |t| t.elapsed().unwrap_or(Duration::MAX) > Duration::from_secs(20 * 3600));
    if due {
        return snapshot(conn, dir, "auto").map(Some);
    }
    Ok(None)
}

/// Check a file is a healthy Task Widget database this version can read. Read-only.
pub fn validate(path: &Path) -> Result<()> {
    let bad = || rule::<()>("That file isn't a valid Task Widget backup.");
    if !path.is_file() {
        return rule("Backup file not found.");
    }
    let Ok(src) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX) else {
        return bad();
    };
    let Ok(check) = src.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)) else { return bad() };
    if check != "ok" {
        return rule("That backup is damaged and can't be restored.");
    }
    let v = db::version(&src).unwrap_or(0);
    if v == 0 {
        return bad();
    }
    if v > db::schema_version() {
        return rule("That backup was made by a newer version of Task Widget. Update the app first.");
    }
    for table in ["tasks", "occurrences", "ledger", "days", "settings"] {
        let ok: bool = src
            .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)", [table], |r| r.get(0))
            .unwrap_or(false);
        if !ok {
            return bad();
        }
    }
    Ok(())
}

/// Replace all live data with the backup at `path`.
/// Steps: validate → back up current data → copy in one step (atomic) → upgrade schema if older.
/// If anything fails before the copy, live data is untouched.
pub fn restore(conn: &mut Connection, path: &Path, dir: &Path) -> Result<PathBuf> {
    validate(path)?;
    let safety = snapshot(conn, dir, "before-restore")?;
    let src = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    {
        let backup = Backup::new(&src, conn)?;
        // -1 = copy every page in a single step, so the swap is all-or-nothing.
        backup.step(-1)?;
    }
    db::migrate(conn)?;
    Ok(safety)
}

/// Resolve a backup name from `list()` to its path, refusing anything outside `dir`.
pub fn path_in(dir: &Path, name: &str) -> Result<PathBuf> {
    if name.contains(['/', '\\']) || name.contains("..") || !name.starts_with(PREFIX) || !name.ends_with(".db") {
        return rule("Unknown backup.");
    }
    Ok(dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Schedule, Size};
    use crate::service::{complete_occurrence, create_task, get_state, NewTask};
    use chrono::NaiveDate;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tw-backup-{tag}-{}-{}", std::process::id(), rand()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn rand() -> u128 {
        SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }
    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
    }
    fn seeded(path: &Path) -> Connection {
        let mut c = db::open(path).unwrap();
        create_task(&mut c, day(), NewTask { title: "Read".into(), details: String::new(), size: Size::Small, schedule: Schedule::Daily, start_date: None }).unwrap();
        let occ = get_state(&mut c, day()).unwrap().cards[0].occurrence_id;
        complete_occurrence(&mut c, day(), "op", occ).unwrap();
        c
    }
    fn balance(c: &mut Connection) -> i64 {
        get_state(c, day()).unwrap().balance
    }

    #[test]
    fn snapshot_then_restore_round_trip() {
        let dir = tmpdir("rt");
        let mut c = seeded(&dir.join("data.db"));
        let snap = snapshot(&c, &dir.join("backups"), "manual").unwrap();
        // Change live data after the snapshot.
        c.execute("INSERT INTO ledger (op_id, kind, coins, xp, created_at) VALUES ('x','correction',500,0,'x')", []).unwrap();
        assert_eq!(balance(&mut c), 510);
        let safety = restore(&mut c, &snap, &dir.join("backups")).unwrap();
        assert_eq!(balance(&mut c), 10);
        assert!(safety.exists(), "current data was saved before restoring");
        // The safety copy holds the pre-restore state.
        validate(&safety).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_or_foreign_files_are_rejected_and_live_data_untouched() {
        let dir = tmpdir("bad");
        let mut c = seeded(&dir.join("data.db"));
        let garbage = dir.join("garbage.db");
        std::fs::write(&garbage, b"this is not a database at all").unwrap();
        assert!(restore(&mut c, &garbage, &dir.join("backups")).is_err());
        let foreign = dir.join("other.db");
        Connection::open(&foreign).unwrap().execute_batch("CREATE TABLE notes(x); PRAGMA user_version = 1;").unwrap();
        assert!(restore(&mut c, &foreign, &dir.join("backups")).is_err());
        assert!(restore(&mut c, &dir.join("missing.db"), &dir.join("backups")).is_err());
        assert_eq!(balance(&mut c), 10);
        assert!(list(&dir.join("backups")).unwrap().is_empty(), "nothing was written for a rejected file");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn newer_version_backup_rejected() {
        let dir = tmpdir("newer");
        let c = seeded(&dir.join("data.db"));
        let snap = snapshot(&c, &dir, "manual").unwrap();
        Connection::open(&snap).unwrap().pragma_update(None, "user_version", 99).unwrap();
        assert!(validate(&snap).is_err());
        // And the app itself refuses to open newer data instead of silently using it.
        assert!(db::open(&snap).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn keeps_only_seven_auto_backups() {
        let dir = tmpdir("prune");
        let c = seeded(&dir.join("data.db"));
        let b = dir.join("backups");
        snapshot(&c, &b, "manual").unwrap();
        for _ in 0..10 {
            snapshot(&c, &b, "auto").unwrap();
        }
        let all = list(&b).unwrap();
        assert_eq!(all.iter().filter(|x| x.kind == "auto").count(), KEEP_AUTO);
        assert_eq!(all.iter().filter(|x| x.kind == "manual").count(), 1);
        snapshot(&c, &b, "pre-upgrade-v1").unwrap();
        assert!(list(&b).unwrap().iter().any(|x| x.kind == "pre-upgrade-v1"));
        // A fresh auto backup exists, so another isn't due.
        assert!(auto_if_due(&c, &b).unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn backup_names_cannot_escape_folder() {
        let d = Path::new("/x/backups");
        assert!(path_in(d, "../data.db").is_err());
        assert!(path_in(d, "taskwidget-1/../../x.db").is_err());
        assert!(path_in(d, "notes.db").is_err());
        assert!(path_in(d, "taskwidget-20261019-101010-auto.db").is_ok());
    }
}
