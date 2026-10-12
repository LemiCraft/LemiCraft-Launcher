use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

// Top-level fields are the Windows release (read by every shipped launcher); other platforms sit under `platforms`
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
    // Kept raw: only this OS' entry is parsed, so a malformed entry for another platform can't break the check
    #[serde(default)]
    platforms: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct PlatformRelease {
    version: String,
    #[serde(rename = "downloadUrl")]
    download_url: String,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    changelog: Vec<String>,
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
    // "installer" (Windows), "in_app" (AppImage / macOS .app via the updater plugin) or "manual" (open download_url)
    mode: &'static str,
    manual: bool,
}

// The AppImage is replaced in place, so its folder must be writable
#[cfg(not(windows))]
fn dir_writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(format!(".lemicraft-write-test-{}", std::process::id()));
    let ok = std::fs::File::create(&probe).is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

// An AppImage sets APPIMAGE; a macOS update swaps the whole .app, so it needs a real bundle, not a read-only
// translocated copy or one running from a mounted disk image
#[cfg(not(windows))]
fn in_app_supported() -> bool {
    if cfg!(target_os = "linux") {
        return std::env::var_os("APPIMAGE")
            .map(std::path::PathBuf::from)
            .and_then(|path| path.parent().map(dir_writable))
            .unwrap_or(false);
    }
    std::env::current_exe()
        .map(|path| {
            let path = path.to_string_lossy();
            path.contains(".app/Contents/MacOS/") && !path.contains("/AppTranslocation/") && !path.starts_with("/Volumes/")
        })
        .unwrap_or(false)
}

#[cfg(windows)]
fn in_app_supported() -> bool {
    false
}

fn pick_update(parsed: VersionApiResponse, current_version: &str) -> Option<UpdateInfo> {
    if cfg!(windows) {
        return Some(UpdateInfo {
            version: parsed.version,
            download_url: parsed.download_url,
            file_size: parsed.file_size,
            sha256: parsed.sha256,
            changelog: parsed.changelog,
            is_required: parsed.is_required,
            release_date: parsed.release_date,
            mode: "installer",
            manual: false,
        });
    }
    let entry = parsed.platforms?.get(std::env::consts::OS)?.clone();
    let release: PlatformRelease = serde_json::from_value(entry).ok()?;
    if !is_newer_version(&release.version, current_version) {
        return None;
    }
    let in_app = in_app_supported();
    Some(UpdateInfo {
        version: release.version,
        download_url: release.download_url,
        file_size: release.file_size,
        sha256: release.sha256,
        changelog: release.changelog,
        // There's nothing to force on a platform with no installer, whatever the server says
        is_required: false,
        release_date: release.release_date,
        mode: if in_app { "in_app" } else { "manual" },
        manual: !in_app,
    })
}

// Dot-split integer comparison like the old launcher's; only each part's leading digits count, so "2.1.0-beta" still compares
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
            // Lets the backend tell this launcher apart from the old C# one when deciding isRequired
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
        if !parsed.success {
            return Ok(None);
        }
        Ok(pick_update(parsed, &current_version).filter(|info| is_newer_version(&info.version, &current_version)))
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

        let default_name = if cfg!(target_os = "macos") { "LemiCraft_Installer.dmg" } else { "LemiCraft_Installer.exe" };
        let file_name = download_url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(default_name);
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

        // Confirmed live: an installer spawned by the running launcher dies with it (its CheckIfAppIsRunning step),
        // even detached/breakaway — so the app exits first via a detached helper instead
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;

            let script_path = std::env::temp_dir().join("lemicraft_update_helper.bat");
            // ping as the delay (`timeout` needs a console); pure ASCII since cmd reads .bat in the OEM code page, so the path is an argument
            std::fs::write(&script_path, "@echo off\r\nping 127.0.0.1 -n 3 >nul\r\nstart \"\" /B %1 /S /R\r\n")
                .map_err(|err| err.to_string())?;

            std::process::Command::new("cmd")
                .arg("/C")
                .raw_arg(format!("\"\"{}\" \"{}\"\"", script_path.display(), temp_path.display()))
                .creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB)
                .spawn()
                .map_err(|err| err.to_string())?;

            let _ = app.emit("update-progress", serde_json::json!({ "percent": 100, "restarting": true }));
            app.exit(0);
        }
        #[cfg(not(windows))]
        open::that(&temp_path).map_err(|err| err.to_string())?;

        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

// The plugin verifies the download against the public key baked into this build (plugins.updater), so the API's sha256 isn't needed
#[cfg(not(windows))]
#[tauri::command]
pub async fn install_update_in_app(app: AppHandle) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};

    // A double click must not start two installs writing to the same file
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Обновление уже выполняется".to_string());
    }
    let result = run_in_app_update(app).await;
    RUNNING.store(false, Ordering::SeqCst);
    result
}

#[cfg(not(windows))]
async fn run_in_app_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;

    let fail = |err: tauri_plugin_updater::Error| {
        use tauri_plugin_updater::Error;
        log::warn!("in-app update failed: {err:?}");
        match &err {
            Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) | Error::SignedVersionMismatch { .. } | Error::MissingSignedVersion => {
                "Проверка подписи не прошла: файл обновления повреждён или подменён".to_string()
            }
            Error::Reqwest(err) if err.is_decode() => {
                "Сервер обновлений вернул неожиданный ответ — скачайте новую версию вручную".to_string()
            }
            Error::Reqwest(_) | Error::Network(_) => {
                "Не удалось связаться с сервером обновлений. Проверьте интернет или VPN и попробуйте снова".to_string()
            }
            Error::ReleaseNotFound | Error::TargetNotFound(_) | Error::TargetsNotFound(_) => {
                "Для вашей системы автоматическое обновление ещё не опубликовано — скачайте новую версию вручную".to_string()
            }
            Error::Io(_) | Error::TempDirNotOnSameMountPoint | Error::FailedToDetermineExtractPath => {
                "Не удалось заменить файл лаунчера. Проверьте, что папка с ним доступна для записи".to_string()
            }
            Error::PreviousAppNotRestored(path) => format!("Обновление не удалось, прежняя версия сохранена здесь: {}", path.display()),
            _ => format!("Не удалось обновить автоматически: {err}"),
        }
    };

    let update = app
        .updater()
        .map_err(fail)?
        .check()
        .await
        .map_err(fail)?
        .ok_or_else(|| "Автоматическое обновление пока недоступно — скачайте новую версию вручную".to_string())?;

    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let percent = total.map_or(-1.0, |total| downloaded as f64 / total as f64 * 100.0);
                let _ = app.emit("update-progress", serde_json::json!({ "percent": percent, "bytes": downloaded }));
            },
            || {},
        )
        .await
        .map_err(fail)?;

    let _ = app.emit("update-progress", serde_json::json!({ "percent": 100, "restarting": true }));
    app.restart()
}

#[cfg(windows)]
#[tauri::command]
pub async fn install_update_in_app(_app: AppHandle) -> Result<(), String> {
    Err("На Windows обновление ставится установщиком".to_string())
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
    // Formatted per byte: sha2 0.11's output type doesn't implement LowerHex
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
