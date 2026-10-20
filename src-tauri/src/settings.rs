//! User settings, stored as key/value rows in the `settings` table.

use crate::service::{rule, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub always_on_top: bool,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "system" | "reduced" | "full"
    pub motion: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { always_on_top: false, theme: "system".into(), motion: "system".into() }
    }
}

fn get(c: &Connection, key: &str) -> Result<Option<String>> {
    Ok(c.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub(crate) fn set(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn load(c: &Connection) -> Result<Settings> {
    let d = Settings::default();
    Ok(Settings {
        always_on_top: get(c, "always_on_top")?.map(|v| v == "1").unwrap_or(d.always_on_top),
        theme: get(c, "theme")?.unwrap_or(d.theme),
        motion: get(c, "motion")?.unwrap_or(d.motion),
    })
}

pub fn save(c: &mut Connection, s: &Settings) -> Result<()> {
    if !["system", "light", "dark"].contains(&s.theme.as_str()) {
        return rule("Unknown theme.");
    }
    if !["system", "reduced", "full"].contains(&s.motion.as_str()) {
        return rule("Unknown motion setting.");
    }
    let tx = c.transaction()?;
    set(&tx, "always_on_top", if s.always_on_top { "1" } else { "0" })?;
    set(&tx, "theme", &s.theme)?;
    set(&tx, "motion", &s.motion)?;
    tx.commit()?;
    Ok(())
}

/// Saved widget position (physical pixels), if any.
pub fn widget_position(c: &Connection) -> Result<Option<(i32, i32)>> {
    let x = get(c, "widget_x")?.and_then(|v| v.parse().ok());
    let y = get(c, "widget_y")?.and_then(|v| v.parse().ok());
    Ok(x.zip(y))
}

pub fn save_widget_position(c: &Connection, x: i32, y: i32) -> Result<()> {
    set(c, "widget_x", &x.to_string())?;
    set(c, "widget_y", &y.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn defaults_save_and_validate() {
        let mut c = db::open_in_memory().unwrap();
        assert_eq!(load(&c).unwrap(), Settings::default());
        let s = Settings { always_on_top: true, theme: "dark".into(), motion: "reduced".into() };
        save(&mut c, &s).unwrap();
        assert_eq!(load(&c).unwrap(), s);
        assert!(save(&mut c, &Settings { theme: "pink".into(), ..s.clone() }).is_err());
        assert_eq!(widget_position(&c).unwrap(), None);
        save_widget_position(&c, -1200, 340).unwrap();
        assert_eq!(widget_position(&c).unwrap(), Some((-1200, 340)));
    }
}
