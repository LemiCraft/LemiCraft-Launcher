// explorer.exe mishandles URLs with real query strings (e.g. ely.by's OAuth URL) — open() doesn't
// https-only since `target` is backend-controlled data — blocks UNC paths/dangerous URI schemes
#[tauri::command]
pub fn open_external(target: String) -> Result<(), String> {
    if !target.starts_with("https://") {
        return Err("Разрешено открывать только https-ссылки".to_string());
    }
    open::that(target).map_err(|err| err.to_string())
}

// For local paths the launcher constructed itself (e.g. the game folder) — never fed
// attacker/backend-controlled input, so the http(s)-only restriction above doesn't apply
pub fn open_local_path(path: String) -> Result<(), String> {
    open::that(path).map_err(|err| err.to_string())
}
