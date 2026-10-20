//! Thin Tauri wrappers: lock the DB, call the service, tell windows to refresh.

use crate::service::{self, AppError, AppState, CompleteResult, NewTask};
use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, State};

pub struct Db(pub Mutex<Connection>);

impl Db {
    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
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
    service::get_state(&mut db.lock(), today())
}

#[tauri::command]
pub fn create_task(app: AppHandle, db: State<Db>, task: NewTask) -> R<i64> {
    let id = service::create_task(&mut db.lock(), today(), task)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn complete_occurrence(app: AppHandle, db: State<Db>, op_id: String, occurrence_id: i64) -> R<CompleteResult> {
    let r = service::complete_occurrence(&mut db.lock(), today(), &op_id, occurrence_id)?;
    changed(&app);
    Ok(r)
}

#[tauri::command]
pub fn uncomplete_occurrence(app: AppHandle, db: State<Db>, op_id: String, occurrence_id: i64) -> R<i64> {
    let b = service::uncomplete_occurrence(&mut db.lock(), today(), &op_id, occurrence_id)?;
    changed(&app);
    Ok(b)
}

#[tauri::command]
pub fn skip_occurrence(app: AppHandle, db: State<Db>, occurrence_id: i64) -> R<()> {
    service::skip_occurrence(&mut db.lock(), today(), occurrence_id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_task(app: AppHandle, db: State<Db>, task_id: i64) -> R<()> {
    service::remove_task(&mut db.lock(), today(), task_id)?;
    changed(&app);
    Ok(())
}

// ---------- phase 5: rewards and revives ----------

use crate::shop::{self, RewardInput};

#[tauri::command]
pub fn buy_revive(app: AppHandle, db: State<Db>, op_id: String) -> R<i64> {
    let b = shop::buy_revive(&mut db.lock(), today(), &op_id)?;
    changed(&app);
    Ok(b)
}

#[tauri::command]
pub fn use_revive(app: AppHandle, db: State<Db>) -> R<i64> {
    let s = shop::use_revive(&mut db.lock(), today())?;
    changed(&app);
    Ok(s)
}

#[tauri::command]
pub fn create_reward(app: AppHandle, db: State<Db>, reward: RewardInput) -> R<i64> {
    let id = shop::create_reward(&mut db.lock(), reward)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn update_reward(app: AppHandle, db: State<Db>, id: i64, reward: RewardInput) -> R<()> {
    shop::update_reward(&mut db.lock(), id, reward)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn archive_reward(app: AppHandle, db: State<Db>, id: i64) -> R<()> {
    shop::archive_reward(&mut db.lock(), id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn redeem_reward(app: AppHandle, db: State<Db>, op_id: String, id: i64) -> R<i64> {
    let b = shop::redeem_reward(&mut db.lock(), today(), &op_id, id)?;
    changed(&app);
    Ok(b)
}
