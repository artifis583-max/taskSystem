mod commands;
pub mod config;
pub mod db;
pub mod domain;
pub mod service;

use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent};

const EDGE_MARGIN: i32 = 6;

fn show_widget(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        let _ = app.emit("state-changed", ());
    }
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
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Task Widget")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => show_widget(app),
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
            std::thread::sleep(Duration::from_secs(30));
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
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("data.db"))?;
            app.manage(commands::Db(Mutex::new(conn)));
            build_tray(app.handle())?;
            if let Some(w) = app.get_webview_window("widget") {
                place_bottom_right(&w);
                w.show()?;
            }
            watch_date(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the widget hides it to the tray; Quit is in the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::create_task,
            commands::complete_occurrence,
            commands::uncomplete_occurrence,
            commands::skip_occurrence,
            commands::remove_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Task Widget");
}
