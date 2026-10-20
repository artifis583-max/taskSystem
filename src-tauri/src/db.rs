//! SQLite setup and schema migrations (tracked with PRAGMA user_version).

use rusqlite::Connection;
use std::path::Path;

const MIGRATIONS: &[&str] = &[
    // 1: initial schema
    r#"
    CREATE TABLE tasks (
        id            INTEGER PRIMARY KEY,
        title         TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 120),
        details       TEXT NOT NULL DEFAULT '',
        category      TEXT NOT NULL DEFAULT '',
        size          TEXT NOT NULL CHECK (size IN ('small','medium','large')),
        coin_override INTEGER,
        xp_override   INTEGER,
        schedule_json TEXT NOT NULL,
        start_date    TEXT NOT NULL,
        end_date      TEXT,
        status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','paused','archived')),
        sort_order    INTEGER NOT NULL DEFAULT 0,
        created_at    TEXT NOT NULL
    );
    CREATE TABLE occurrences (
        id             INTEGER PRIMARY KEY,
        task_id        INTEGER NOT NULL REFERENCES tasks(id),
        original_date  TEXT NOT NULL,
        due_date       TEXT NOT NULL,
        status         TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','completed','skipped','cancelled')),
        rollover_count INTEGER NOT NULL DEFAULT 0,
        completed_at   TEXT,
        awarded_coins  INTEGER,
        awarded_xp     INTEGER,
        completed_seq  INTEGER,
        UNIQUE (task_id, original_date)
    );
    CREATE INDEX occ_due ON occurrences(due_date, status);
    CREATE TABLE ledger (
        id            INTEGER PRIMARY KEY,
        op_id         TEXT NOT NULL UNIQUE,
        kind          TEXT NOT NULL CHECK (kind IN ('task','undo','revive_buy','redeem','correction')),
        coins         INTEGER NOT NULL,
        xp            INTEGER NOT NULL,
        occurrence_id INTEGER REFERENCES occurrences(id),
        created_at    TEXT NOT NULL
    );
    CREATE TABLE days (
        date         TEXT PRIMARY KEY,
        eligible     INTEGER NOT NULL,
        completed    INTEGER NOT NULL,
        result       TEXT NOT NULL CHECK (result IN ('qualified','neutral','broken','revived')),
        streak_after INTEGER NOT NULL,
        finalized_at TEXT NOT NULL
    );
    CREATE TABLE revives (
        id            INTEGER PRIMARY KEY,
        source        TEXT NOT NULL CHECK (source IN ('free','bought')),
        week_of       TEXT,
        used_for_date TEXT UNIQUE,
        used_at       TEXT
    );
    CREATE TABLE rewards (
        id       INTEGER PRIMARY KEY,
        name     TEXT NOT NULL,
        note     TEXT NOT NULL DEFAULT '',
        price    INTEGER NOT NULL CHECK (price > 0),
        archived INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE redemptions (
        id         INTEGER PRIMARY KEY,
        reward_id  INTEGER NOT NULL REFERENCES rewards(id),
        price_paid INTEGER NOT NULL,
        ledger_id  INTEGER NOT NULL REFERENCES ledger(id),
        created_at TEXT NOT NULL
    );
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
];

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let current: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i + 1)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn schema_version() -> usize {
    MIGRATIONS.len()
}
