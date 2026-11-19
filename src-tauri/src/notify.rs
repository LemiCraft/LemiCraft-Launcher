// System notifications and the start with the system
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

/// Passed by the system's autostart entry: the launcher then opens minimized
pub const AUTOSTART_ARG: &str = "--autostart";

pub fn started_by_system() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

/// Shown only when the player is not looking at the launcher: what it says is on the screen otherwise
#[tauri::command]
pub fn show_notification(app: AppHandle, title: String, body: String, force: Option<bool>) -> Result<(), String> {
    let force = force.unwrap_or(false);
    if !force && !crate::settings::load().notifications {
        return Ok(());
    }
    if let (false, Some(window)) = (force, app.get_webview_window("main")) {
        let in_front = window.is_focused().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
        if in_front {
            return Ok(());
        }
    }
    app.notification().builder().title(title).body(body).show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|err| format!("Не удалось изменить автозапуск: {err}"))
}
