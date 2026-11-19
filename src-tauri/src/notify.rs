// System notifications and the start with the system
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

/// Passed by the system's autostart entry: the launcher then opens minimized
pub const AUTOSTART_ARG: &str = "--autostart";

pub fn started_by_system() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

/// Windows shows a notification under the name and icon of the application ID it is sent with, and one that is
/// not installed has none registered. It is written to the user's own registry branch, so no rights are needed.
#[cfg(windows)]
pub fn register_app_identity(app: &AppHandle) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let id = app.config().identifier.clone();
    std::thread::spawn(move || {
        let icon = crate::game::game_dir().join("notify-icon.ico");
        if !icon.is_file() {
            let _ = std::fs::create_dir_all(crate::game::game_dir());
            let _ = std::fs::write(&icon, include_bytes!("../icons/icon.ico"));
        }
        let key = format!("HKCU\\Software\\Classes\\AppUserModelId\\{id}");
        for (name, value) in [("DisplayName", "LemiCraft".to_string()), ("IconUri", icon.display().to_string())] {
            let _ = std::process::Command::new("reg")
                .args(["add", &key, "/v", name, "/t", "REG_SZ", "/d", &value, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    });
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

#[cfg(windows)]
fn cached_picture(url: &str) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    const MAX_BYTES: u64 = 3 * 1024 * 1024;
    if !url.starts_with("https://cdn.discordapp.com/") {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let extension = if url.split('?').next().unwrap_or("").to_ascii_lowercase().ends_with(".jpg") { "jpg" } else { "png" };
    let dir = crate::game::game_dir().join("notify-cache");
    let path = dir.join(format!("{:x}.{extension}", hasher.finish()));
    if path.is_file() {
        return Some(path);
    }
    std::fs::create_dir_all(&dir).ok()?;
    let res = crate::http::blocking().get(url).send().ok()?;
    if !res.status().is_success() || res.content_length().is_some_and(|len| len > MAX_BYTES) {
        return None;
    }
    let bytes = res.bytes().ok()?;
    (bytes.len() as u64 <= MAX_BYTES).then(|| std::fs::write(&path, &bytes).ok()).flatten()?;
    Some(path)
}

/// Brings the launcher up from the tray, the taskbar or behind other windows
fn bring_to_front(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn launcher_in_front(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|w| w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
}

/// A new post: the author as the title, the name of the post as the text, the cover on top. A click brings the
/// launcher up and opens the post (Windows; the other systems get a plain notification).
#[tauri::command]
pub async fn show_news_toast(app: AppHandle, id: String, title: String, body: String, picture_url: Option<String>, force: Option<bool>) -> Result<(), String> {
    let force = force.unwrap_or(false);
    if !force && (!crate::settings::load().notifications || launcher_in_front(&app)) {
        return Ok(());
    }
    #[cfg(windows)]
    {
        use tauri::Emitter;
        let identity = app.config().identifier.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut toast = tauri_winrt_notification::Toast::new(&identity).title(&title).text1(&body);
            if let Some(path) = picture_url.as_deref().and_then(cached_picture) {
                toast = toast.hero(&path, "");
            }
            let handle = app.clone();
            toast
                .on_activated(move |_| {
                    bring_to_front(&handle);
                    let _ = handle.emit("open-news", id.clone());
                    Ok(())
                })
                .show()
                .map_err(|err| err.to_string())
        })
        .await
        .map_err(|err| err.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = (id, picture_url);
        app.notification().builder().title(title).body(body).show().map_err(|err| err.to_string())
    }
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
