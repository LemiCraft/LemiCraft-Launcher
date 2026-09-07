use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Deserialize)]
struct VersionApiResponse {
    success: bool,
    version: String,
    #[serde(rename = "downloadUrl")]
    download_url: String,
    #[serde(rename = "fileSize")]
    file_size: u64,
    sha256: Option<String>,
    #[serde(default)]
    changelog: Vec<String>,
    #[serde(rename = "isRequired", default)]
    is_required: bool,
    #[serde(rename = "releaseDate", default)]
    release_date: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    version: String,
    download_url: String,
    file_size: u64,
    sha256: Option<String>,
    changelog: Vec<String>,
    is_required: bool,
    release_date: Option<String>,
}

// Dot-split integer comparison, not semver-aware — matches the old launcher's own check.
// Only the leading digit run of each part is used, so a suffix like "2.1.0-beta" still compares
fn is_newer_version(new_version: &str, current_version: &str) -> bool {
    let parse = |v: &str| -> Vec<i64> {
        v.split('.')
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<i64>().unwrap_or(0))
            .collect()
    };
    let new_parts = parse(new_version);
    let current_parts = parse(current_version);
    for i in 0..new_parts.len().min(current_parts.len()) {
        if new_parts[i] > current_parts[i] {
            return true;
        }
        if new_parts[i] < current_parts[i] {
            return false;
        }
    }
    new_parts.len() > current_parts.len()
}

#[tauri::command]
pub async fn check_for_update(current_version: String) -> Result<Option<UpdateInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            // Lets the backend tell this launcher apart from the old C# one when deciding
            // isRequired — see update.rs's own check_for_update vs. the old launcher's UpdateService
            .user_agent(format!("LemiCraft-Launcher/{current_version} (Tauri)"))
            .build()
            .map_err(|err| err.to_string())?;
        let res = client
            .get(format!("{}/launcher/version", crate::config::API_BASE))
            .send()
            .map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Ok(None);
        }
        let parsed: VersionApiResponse = res.json().map_err(|err| err.to_string())?;
        if !parsed.success || !is_newer_version(&parsed.version, &current_version) {
            return Ok(None);
        }
        Ok(Some(UpdateInfo {
            version: parsed.version,
            download_url: parsed.download_url,
            file_size: parsed.file_size,
            sha256: parsed.sha256,
            changelog: parsed.changelog,
            is_required: parsed.is_required,
            release_date: parsed.release_date,
        }))
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn download_and_install_update(app: AppHandle, download_url: String, sha256: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::{Read, Write};

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|err| err.to_string())?;

        let mut response = client.get(&download_url).send().map_err(|err| err.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Не удалось скачать обновление ({})", response.status()));
        }
        let total = response.content_length().unwrap_or(0);

        let file_name = download_url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("LemiCraft_Installer.exe");
        let temp_path = std::env::temp_dir().join(file_name);
        let mut file = std::fs::File::create(&temp_path).map_err(|err| err.to_string())?;

        let mut downloaded: u64 = 0;
        let mut buf = [0u8; 81920];
        loop {
            let n = response.read(&mut buf).map_err(|err| err.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|err| err.to_string())?;
            downloaded += n as u64;
            let percent = if total > 0 { (downloaded as f64 / total as f64) * 100.0 } else { -1.0 };
            let _ = app.emit("update-progress", serde_json::json!({ "percent": percent, "bytes": downloaded }));
        }
        drop(file);

        // A missing/empty hash must block the install, not silently skip verification
        let Some(expected) = sha256.filter(|s| !s.is_empty()) else {
            let _ = std::fs::remove_file(&temp_path);
            return Err("Сервер не прислал контрольную сумму (SHA-256) для обновления — установка отменена в целях безопасности".to_string());
        };
        let actual = sha256_hex(&temp_path).map_err(|err| err.to_string())?;
        if !actual.eq_ignore_ascii_case(&expected) {
            let _ = std::fs::remove_file(&temp_path);
            return Err("Проверка целостности скачанного файла не прошла (SHA-256 не совпадает)".to_string());
        }

        let _ = app.emit("update-progress", serde_json::json!({ "percent": 100, "bytes": downloaded }));

        // NSIS silent-install flag — the old launcher's Inno Setup flags (/SILENT etc.) don't apply here.
        // DETACHED_PROCESS alone wasn't enough (confirmed live) — it only detaches the console, not
        // Job Object membership, so a kill-on-close job (likely from WebView2) still took the installer down
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
            // /R: Tauri's NSIS template relaunches the app after a silent install completes —
            // without it the update finishes with no launcher window open at all
            std::process::Command::new(&temp_path)
                .arg("/S")
                .arg("/R")
                .creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB)
                .spawn()
                .map_err(|err| err.to_string())?;
        }
        #[cfg(not(windows))]
        open::that(&temp_path).map_err(|err| err.to_string())?;

        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

fn sha256_hex(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|err| err.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    // Formatted byte-by-byte rather than via `{:x}` on the digest directly — sha2 0.11 switched
    // its output type (generic-array -> hybrid-array) and the new type doesn't implement LowerHex
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
