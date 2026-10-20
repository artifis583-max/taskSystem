//! Thin Tauri wrappers: lock the DB, call the service, tell windows to refresh.

use crate::service::{self, AppError, AppState, CompleteResult, NewTask};
use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, State};

pub struct Db(pub Mutex<Connection>);

/// Set when the data file couldn't be opened at startup. Every command then refuses
/// to run, so nothing is ever written to a placeholder database.
pub static STARTUP_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

impl Db {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn conn(&self) -> Result<MutexGuard<'_, Connection>, AppError> {
        if let Some(e) = STARTUP_ERROR.get() {
            return Err(AppError::Rule(format!("Task Widget couldn't open your data, so nothing was changed. {e}")));
        }
        Ok(self.lock())
    }
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn changed(app: &AppHandle) {
    let _ = app.emit("state-changed", ());
}

type R<T> = Result<T, AppError>;

#[tauri::command]
pub fn get_state(db: State<Db>) -> R<AppState> {
    service::get_state(&mut *db.conn()?, today())
}

#[tauri::command]
pub fn create_task(app: AppHandle, db: State<Db>, task: NewTask) -> R<i64> {
    let id = service::create_task(&mut *db.conn()?, today(), task)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn complete_occurrence(app: AppHandle, db: State<Db>, op_id: String, occurrence_id: i64) -> R<CompleteResult> {
    let r = service::complete_occurrence(&mut *db.conn()?, today(), &op_id, occurrence_id)?;
    changed(&app);
    Ok(r)
}

#[tauri::command]
pub fn uncomplete_occurrence(app: AppHandle, db: State<Db>, op_id: String, occurrence_id: i64) -> R<i64> {
    let b = service::uncomplete_occurrence(&mut *db.conn()?, today(), &op_id, occurrence_id)?;
    changed(&app);
    Ok(b)
}

#[tauri::command]
pub fn skip_occurrence(app: AppHandle, db: State<Db>, occurrence_id: i64) -> R<()> {
    service::skip_occurrence(&mut *db.conn()?, today(), occurrence_id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_task(app: AppHandle, db: State<Db>, task_id: i64) -> R<()> {
    service::remove_task(&mut *db.conn()?, today(), task_id)?;
    changed(&app);
    Ok(())
}

// ---------- phase 5: rewards and revives ----------

use crate::shop::{self, RewardInput};

#[tauri::command]
pub fn buy_revive(app: AppHandle, db: State<Db>, op_id: String) -> R<i64> {
    let b = shop::buy_revive(&mut *db.conn()?, today(), &op_id)?;
    changed(&app);
    Ok(b)
}

#[tauri::command]
pub fn use_revive(app: AppHandle, db: State<Db>) -> R<i64> {
    let s = shop::use_revive(&mut *db.conn()?, today())?;
    changed(&app);
    Ok(s)
}

#[tauri::command]
pub fn create_reward(app: AppHandle, db: State<Db>, reward: RewardInput) -> R<i64> {
    let id = shop::create_reward(&mut *db.conn()?, reward)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn update_reward(app: AppHandle, db: State<Db>, id: i64, reward: RewardInput) -> R<()> {
    shop::update_reward(&mut *db.conn()?, id, reward)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn archive_reward(app: AppHandle, db: State<Db>, id: i64) -> R<()> {
    shop::archive_reward(&mut *db.conn()?, id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn redeem_reward(app: AppHandle, db: State<Db>, op_id: String, id: i64) -> R<i64> {
    let b = shop::redeem_reward(&mut *db.conn()?, today(), &op_id, id)?;
    changed(&app);
    Ok(b)
}

// ---------- phase 6: dashboard and settings ----------

use crate::manage::{self, DayInfo, Stats, TaskInfo};
use crate::settings::{self, Settings};
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn list_tasks(db: State<Db>) -> R<Vec<TaskInfo>> {
    manage::list_tasks(&*db.conn()?)
}

#[tauri::command]
pub fn update_task(app: AppHandle, db: State<Db>, id: i64, task: NewTask) -> R<()> {
    manage::update_task(&mut *db.conn()?, today(), id, task)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_task_status(app: AppHandle, db: State<Db>, id: i64, status: String) -> R<()> {
    manage::set_task_status(&mut *db.conn()?, today(), id, &status)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn get_history(db: State<Db>) -> R<Vec<DayInfo>> {
    let mut c = db.conn()?;
    service::sync_day(&mut c, today())?;
    manage::history(&c, 120)
}

#[tauri::command]
pub fn get_stats(db: State<Db>) -> R<Stats> {
    manage::stats(&mut *db.conn()?, today())
}

#[tauri::command]
pub fn update_settings(app: AppHandle, db: State<Db>, settings: Settings) -> R<()> {
    settings::save(&mut *db.conn()?, &settings)?;
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.set_always_on_top(settings.always_on_top);
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> R<bool> {
    let al = app.autolaunch();
    let res = if enabled { al.enable() } else { al.disable() };
    res.map_err(|e| AppError::Rule(format!("Couldn't change the startup setting: {e}")))?;
    Ok(al.is_enabled().unwrap_or(false))
}

#[tauri::command]
pub fn open_dashboard(app: AppHandle) -> R<()> {
    crate::open_dashboard(&app).map_err(|e| AppError::Rule(format!("Couldn't open the dashboard: {e}")))
}

// ---------- phase 7: backups ----------

use crate::backup::{self, BackupFile};
use std::path::PathBuf;
use tauri_plugin_dialog::DialogExt;

pub struct Paths {
    pub data_dir: PathBuf,
    pub backups: PathBuf,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupsInfo {
    pub data_dir: String,
    pub backups_dir: String,
    pub backups: Vec<BackupFile>,
}

#[tauri::command]
pub fn backups_info(paths: State<Paths>) -> R<BackupsInfo> {
    Ok(BackupsInfo {
        data_dir: paths.data_dir.to_string_lossy().into(),
        backups_dir: paths.backups.to_string_lossy().into(),
        backups: backup::list(&paths.backups)?,
    })
}

#[tauri::command]
pub fn backup_now(app: AppHandle, db: State<Db>, paths: State<Paths>) -> R<String> {
    let p = backup::snapshot(&*db.conn()?, &paths.backups, "manual")?;
    changed(&app);
    Ok(p.file_name().unwrap_or_default().to_string_lossy().into())
}

/// Save a copy wherever the user picks. Returns false if they cancelled.
#[tauri::command]
pub async fn export_backup(app: AppHandle) -> R<bool> {
    let name = format!("taskwidget-backup-{}.db", today());
    let Some(fp) = app.dialog().file().set_file_name(&name).add_filter("Task Widget backup", &["db"]).blocking_save_file() else {
        return Ok(false);
    };
    let path = fp.into_path().map_err(|e| AppError::Rule(format!("Can't use that location: {e}")))?;
    let db = app.state::<Db>();
    backup::write_copy(&*db.conn()?, &path)?;
    Ok(true)
}

fn restore_path(app: &AppHandle, path: &std::path::Path) -> R<()> {
    let db = app.state::<Db>();
    let paths = app.state::<Paths>();
    backup::restore(&mut *db.conn()?, path, &paths.backups)?;
    changed(app);
    Ok(())
}

/// Restore one of the backups in the app's backups folder.
#[tauri::command]
pub fn restore_backup(app: AppHandle, name: String) -> R<()> {
    let path = backup::path_in(&app.state::<Paths>().backups, &name)?;
    restore_path(&app, &path)
}

/// Pick a backup file anywhere and restore it. Returns false if they cancelled.
#[tauri::command]
pub async fn restore_from_file(app: AppHandle) -> R<bool> {
    let start = app.state::<Paths>().backups.clone();
    let Some(fp) = app.dialog().file().set_directory(start).add_filter("Task Widget backup", &["db"]).blocking_pick_file() else {
        return Ok(false);
    };
    let path = fp.into_path().map_err(|e| AppError::Rule(format!("Can't open that file: {e}")))?;
    restore_path(&app, &path)?;
    Ok(true)
}
