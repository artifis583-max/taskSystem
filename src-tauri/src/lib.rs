mod commands;
pub mod config;
pub mod db;
pub mod domain;
pub mod manage;
pub mod service;
pub mod settings;
pub mod shop;

use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use tauri::menu::PredefinedMenuItem;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

const EDGE_MARGIN: i32 = 6;

fn show_widget(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        let _ = app.emit("state-changed", ());
    }
}

/// Open (or focus) the dashboard window.
pub(crate) fn open_dashboard(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("dashboard") {
        w.show()?;
        w.unminimize()?;
        return w.set_focus();
    }
    WebviewWindowBuilder::new(app, "dashboard", WebviewUrl::App("index.html?view=dashboard".into()))
        .title("Task Widget")
        .inner_size(920.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .center()
        .build()?;
    Ok(())
}

// Latest widget position from drag events. Written to the DB a few seconds later
// (and on exit) instead of on every mouse move.
static POS_X: AtomicI32 = AtomicI32::new(0);
static POS_Y: AtomicI32 = AtomicI32::new(0);
static POS_DIRTY: AtomicBool = AtomicBool::new(false);

fn flush_position(app: &AppHandle) {
    if POS_DIRTY.swap(false, Ordering::SeqCst) {
        if let Some(db) = app.try_state::<commands::Db>() {
            let c = db.0.lock().unwrap_or_else(|e| e.into_inner());
            let _ = settings::save_widget_position(&c, POS_X.load(Ordering::SeqCst), POS_Y.load(Ordering::SeqCst));
        }
    }
}

/// Restore the saved position if its top-left area is still on a connected monitor;
/// otherwise (first run, monitor unplugged) use the bottom-right corner.
fn place_widget(w: &WebviewWindow, saved: Option<(i32, i32)>) {
    if let Some((x, y)) = saved {
        let visible = w.available_monitors().unwrap_or_default().iter().any(|m| {
            let a = m.work_area();
            let (ax, ay) = (a.position.x, a.position.y);
            let (aw, ah) = (a.size.width as i32, a.size.height as i32);
            // At least the header's left part must be reachable.
            x + 120 > ax && x < ax + aw - 40 && y >= ay - 10 && y < ay + ah - 40
        });
        if visible {
            let _ = w.set_position(PhysicalPosition::new(x, y));
            return;
        }
    }
    place_bottom_right(w);
}

/// Put the widget in the bottom-right corner of the primary monitor's work area (above the taskbar).
fn place_bottom_right(w: &WebviewWindow) {
    let Ok(Some(monitor)) = w.primary_monitor() else { return };
    let Ok(size) = w.outer_size() else { return };
    let area = monitor.work_area();
    let x = area.position.x + area.size.width as i32 - size.width as i32 - EDGE_MARGIN;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - EDGE_MARGIN;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show widget", true, None::<&str>)?;
    let dash = MenuItem::with_id(app, "dashboard", "Open dashboard", true, None::<&str>)?;
    let add = MenuItem::with_id(app, "add", "Add task", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &dash, &add, &sep, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Task Widget")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => show_widget(app),
            "dashboard" => {
                let _ = open_dashboard(app);
            }
            "add" => {
                show_widget(app);
                let _ = app.emit_to("widget", "open-add-task", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_widget(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Tell the UI when the local date changes (midnight, or waking from sleep on a new day).
/// The UI then re-fetches state, which finalizes the past day(s).
fn watch_date(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = commands::today();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            flush_position(&app);
            let now = commands::today();
            if now != last {
                last = now;
                let _ = app.emit("state-changed", ());
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_widget(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("data.db"))?;
            let prefs = settings::load(&conn)?;
            let saved_pos = settings::widget_position(&conn)?;
            app.manage(commands::Db(Mutex::new(conn)));
            build_tray(app.handle())?;
            if let Some(w) = app.get_webview_window("widget") {
                place_widget(&w, saved_pos);
                w.set_always_on_top(prefs.always_on_top)?;
                w.show()?;
            }
            watch_date(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "widget" {
                return; // the dashboard closes normally
            }
            match event {
                // Closing the widget hides it to the tray; Quit is in the tray menu.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                    flush_position(window.app_handle());
                }
                WindowEvent::Moved(p) => {
                    POS_X.store(p.x, Ordering::SeqCst);
                    POS_Y.store(p.y, Ordering::SeqCst);
                    POS_DIRTY.store(true, Ordering::SeqCst);
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::create_task,
            commands::complete_occurrence,
            commands::uncomplete_occurrence,
            commands::skip_occurrence,
            commands::remove_task,
            commands::buy_revive,
            commands::use_revive,
            commands::create_reward,
            commands::update_reward,
            commands::archive_reward,
            commands::redeem_reward,
            commands::list_tasks,
            commands::update_task,
            commands::set_task_status,
            commands::get_history,
            commands::get_stats,
            commands::update_settings,
            commands::get_autostart,
            commands::set_autostart,
            commands::open_dashboard,
        ])
        .build(tauri::generate_context!())
        .expect("error while starting Task Widget")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                flush_position(app);
            }
        });
}
