use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};

use portablemc::{base, fabric, moj};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct GameProcess(pub Arc<Mutex<Option<Child>>>);

#[tauri::command]
pub fn is_game_running(process: tauri::State<GameProcess>) -> bool {
    process.0.lock().map(|guard| guard.is_some()).unwrap_or(false)
}

fn current_log_path() -> PathBuf {
    minecraft_dir().join("logs").join("latest.log")
}

#[tauri::command]
pub fn read_current_log() -> String {
    std::fs::read_to_string(current_log_path()).unwrap_or_default()
}

#[tauri::command]
pub async fn move_game_files(from: String, to: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let from = PathBuf::from(from);
        let to = PathBuf::from(to);
        if !from.exists() || from == to {
            return Ok(());
        }
        std::fs::create_dir_all(&to).map_err(|err| err.to_string())?;
        let options = fs_extra::dir::CopyOptions::new().content_only(true).overwrite(true);
        fs_extra::dir::move_dir(&from, &to, &options).map_err(|err| err.to_string())?;
        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn stop_game(app: AppHandle, process: tauri::State<GameProcess>) -> Result<(), String> {
    let mut guard = process.0.lock().map_err(|_| "Не удалось получить доступ к процессу игры".to_string())?;
    let result = match guard.take() {
        Some(mut child) => child.kill().map_err(|err| err.to_string()),
        None => Ok(()),
    };
    drop(guard);

    // spawn_with_log_streaming's wait loop finds the slot already empty and exits silently
    let _ = app.emit("game-stopped", ());
    restore_main_window(&app, &crate::settings::load().on_launch);

    result
}

// Own config (settings, account, ely.by session) always lives here, never the customizable
// minecraft_dir() below. Debug builds use a separate folder so dev never touches a real install;
// release must match the old C# launcher's folder for old_launcher_profile_path() (auth.rs) to work.
pub(crate) fn game_dir() -> PathBuf {
    let folder = if cfg!(debug_assertions) { "LemiCraftLauncherDev" } else { "LemiCraft" };
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join(folder)
}

pub(crate) fn minecraft_dir() -> PathBuf {
    match crate::settings::load().game_dir {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => game_dir(),
    }
}

#[tauri::command]
pub fn get_default_game_dir() -> String {
    game_dir().to_string_lossy().to_string()
}

#[tauri::command]
pub fn is_game_installed() -> bool {
    let version_name = format!("fabric-{}-{}", crate::config::MC_VERSION, crate::config::FABRIC_LOADER);
    minecraft_dir()
        .join("versions")
        .join(&version_name)
        .join(format!("{version_name}.json"))
        .is_file()
}

#[tauri::command]
pub fn open_game_folder() -> Result<(), String> {
    let dir = minecraft_dir();
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.to_string_lossy().to_string();
    crate::shell::open_local_path(path)
}

struct ProgressHandler<'a> {
    app: &'a AppHandle,
}

impl<'a> ProgressHandler<'a> {
    fn emit(&self, payload: serde_json::Value) {
        let _ = self.app.emit("install-progress", payload);
    }

    fn handle_base_event(&self, event: &base::Event) {
        match event {
            base::Event::DownloadProgress { count, total_count, size, total_size } => {
                self.emit(serde_json::json!({
                    "stage": "download",
                    "count": count,
                    "totalCount": total_count,
                    "size": size,
                    "totalSize": total_size,
                }));
            }
            base::Event::LoadVersion { .. } => self.emit(serde_json::json!({ "stage": "version" })),
            base::Event::LoadLibraries { .. } => self.emit(serde_json::json!({ "stage": "libraries" })),
            base::Event::LoadAssets { .. } => self.emit(serde_json::json!({ "stage": "assets" })),
            base::Event::LoadJvm { .. } => self.emit(serde_json::json!({ "stage": "jvm" })),
            _ => {}
        }
    }
}

impl<'a> fabric::Handler for ProgressHandler<'a> {
    fn on_event(&mut self, event: fabric::Event) {
        if let fabric::Event::Mojang(moj::Event::Base(base_event)) = &event {
            self.handle_base_event(base_event);
        }
    }
}

#[tauri::command]
pub fn play(app: AppHandle, username: String, process: tauri::State<GameProcess>) -> Result<(), String> {
    let process = process.0.clone();
    std::thread::spawn(move || match run(&app, username, process) {
        Ok(()) => {
            let _ = app.emit("install-progress", serde_json::json!({ "stage": "launched" }));
        }
        Err(err) => {
            let _ = app.emit("install-error", err);
        }
    });

    Ok(())
}

// A parallel ~5k-file download routinely drops a small handful; a retried install() just fills the gaps
const MAX_INSTALL_ATTEMPTS: u32 = 4;

fn run(app: &AppHandle, username: String, process: Arc<Mutex<Option<Child>>>) -> Result<(), String> {
    let mut last_error = String::new();
    let settings = crate::settings::load();

    for attempt in 1..=MAX_INSTALL_ATTEMPTS {
        if attempt > 1 {
            let _ = app.emit(
                "install-progress",
                serde_json::json!({ "stage": "retry", "attempt": attempt, "maxAttempts": MAX_INSTALL_ATTEMPTS }),
            );
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        let mut installer = fabric::Installer::new(
            fabric::Loader::Fabric,
            fabric::GameVersion::Name(crate::config::MC_VERSION.to_string()),
            fabric::LoaderVersion::Name(crate::config::FABRIC_LOADER.to_string()),
        );

        // Built as an offline session with the real uuid/username, then the real access token is patched in below
        let real_session = crate::auth::load_current_session_full();

        match &real_session {
            Some((uuid, username, _token, _provider)) => {
                let uuid = uuid::Uuid::parse_str(uuid).map_err(|err| err.to_string())?;
                installer.mojang_mut().set_auth_offline(uuid, username.clone());
            }
            None => {
                installer.mojang_mut().set_auth_offline_username(username.clone());
            }
        }
        installer.mojang_mut().base_mut().set_main_dir(minecraft_dir());

        if settings.auto_connect {
            installer.mojang_mut().set_quick_play(moj::QuickPlay::Multiplayer {
                host: crate::config::SERVER_SRV_DOMAIN.to_string(),
                port: crate::config::SERVER_PORT,
            });
        }

        let handler = ProgressHandler { app };
        match installer.install(handler) {
            Ok(mut game) => {
                for dir in ["mods", "config", "resourcepacks", "shaderpacks"] {
                    let _ = std::fs::create_dir_all(minecraft_dir().join(dir));
                }
                crate::server_list::ensure_lemicraft_server_entry();

                if let Some((_, _, access_token, provider)) = &real_session {
                    patch_real_session_args(&mut game.game_args, access_token);

                    if provider == "Ely.by" {
                        let _ = app.emit("install-progress", serde_json::json!({ "stage": "authlib" }));
                        let authlib_path = ensure_authlib_injector()?;
                        game.jvm_args.insert(0, format!("-javaagent:{}=ely.by", authlib_path.to_string_lossy()));
                    }
                }

                game.jvm_args.push(format!("-Xmx{}G", settings.ram_gb));
                game.jvm_args.push(format!("-Xms{}G", settings.ram_gb.min(2)));
                game.jvm_args.extend(
                    settings
                        .jvm_args
                        .split_whitespace()
                        .map(|arg| arg.to_string()),
                );

                spawn_with_log_streaming(app, game.command(), process, &settings.on_launch)?;
                return Ok(());
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }
    }

    Err(format!(
        "Не удалось установить игру после {MAX_INSTALL_ATTEMPTS} попыток: {last_error}"
    ))
}

fn authlib_injector_path() -> PathBuf {
    game_dir().join("authlib-injector.jar")
}

#[derive(serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

// jar files are zip archives — a real download always starts with this magic; a partial/
// corrupted write does not, so this catches a truncated file before it's trusted forever
fn is_valid_jar(bytes: &[u8]) -> bool {
    bytes.len() > 4 && &bytes[..2] == b"PK"
}

fn ensure_authlib_injector() -> Result<PathBuf, String> {
    let path = authlib_injector_path();
    if path.exists() {
        if std::fs::read(&path).map(|bytes| is_valid_jar(&bytes)).unwrap_or(false) {
            return Ok(path);
        }
        let _ = std::fs::remove_file(&path);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| err.to_string())?;

    let release: GhRelease = client
        .get("https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest")
        .header("User-Agent", "LemiCraft-Launcher")
        .send()
        .map_err(|err| err.to_string())?
        .json()
        .map_err(|err| err.to_string())?;

    let short_tag = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    let direct_url = format!(
        "https://github.com/yushijinhun/authlib-injector/releases/download/{}/authlib-injector-{short_tag}.jar",
        release.tag_name
    );

    let bytes = match client.get(&direct_url).send().and_then(|res| res.error_for_status()) {
        Ok(res) => res.bytes().map_err(|err| err.to_string())?,
        Err(_) => {
            let asset = release
                .assets
                .iter()
                .find(|a| a.name.contains("authlib-injector") && a.name.ends_with(".jar"))
                .ok_or("Не удалось найти authlib-injector.jar в последнем релизе на GitHub")?;
            client
                .get(&asset.browser_download_url)
                .send()
                .map_err(|err| err.to_string())?
                .bytes()
                .map_err(|err| err.to_string())?
        }
    };

    if !is_valid_jar(&bytes) {
        return Err("Скачанный authlib-injector.jar повреждён (не является корректным jar-файлом)".to_string());
    }

    std::fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    std::fs::write(&path, &bytes).map_err(|err| err.to_string())?;
    Ok(path)
}

// Wait loop polls try_wait() instead of blocking .wait() so stop_game can grab the mutex too
fn spawn_with_log_streaming(
    app: &AppHandle,
    mut command: std::process::Command,
    process: Arc<Mutex<Option<Child>>>,
    on_launch: &str,
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|err| err.to_string())?;

    if let Some(stdout) = child.stdout.take() {
        let app = app.clone();
        std::thread::spawn(move || {
            let mut filter = XmlLogFilter::new();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(cleaned) = filter.feed(&line) {
                    let _ = app.emit("game-log", cleaned);
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        std::thread::spawn(move || {
            let mut filter = XmlLogFilter::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Some(cleaned) = filter.feed(&line) {
                    let _ = app.emit("game-log", cleaned);
                }
            }
        });
    }

    *process.lock().map_err(|_| "Не удалось сохранить процесс игры".to_string())? = Some(child);
    let _ = app.emit("game-started", ());

    match on_launch {
        "close" => app.exit(0),
        "hide" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        _ => {}
    }

    let on_launch = on_launch.to_string();
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mut guard = match process.lock() {
            Ok(guard) => guard,
            Err(_) => break,
        };
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    drop(guard);
                    let _ = app.emit("game-stopped", ());
                    if !status.success() {
                        let _ = app.emit("game-crashed", status.code());
                    }
                    restore_main_window(&app, &on_launch);
                    break;
                }
                Ok(None) => {}
                Err(_) => {
                    *guard = None;
                    drop(guard);
                    let _ = app.emit("game-stopped", ());
                    restore_main_window(&app, &on_launch);
                    break;
                }
            },
            None => break,
        }
    });

    Ok(())
}

fn restore_main_window(app: &AppHandle, on_launch: &str) {
    if on_launch == "hide" {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

// Buffers Minecraft's <log4j:Event> XML blocks and collapses each into one readable line
struct XmlLogFilter {
    buffer: String,
    in_block: bool,
}

impl XmlLogFilter {
    fn new() -> Self {
        Self { buffer: String::new(), in_block: false }
    }

    fn feed(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<log4j:Event") {
            self.in_block = true;
            self.buffer.clear();
            self.buffer.push_str(line);
            return None;
        }
        if self.in_block {
            self.buffer.push('\n');
            self.buffer.push_str(line);
            if trimmed.starts_with("</log4j:Event>") {
                self.in_block = false;
                return Some(format_log4j_event(&self.buffer));
            }
            return None;
        }
        Some(line.to_string())
    }
}

fn xml_attr<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = xml.find(&needle)? + needle.len();
    let len = xml[start..].find('"')?;
    Some(&xml[start..start + len])
}

fn format_log4j_event(xml: &str) -> String {
    let thread = xml_attr(xml, "thread").unwrap_or("?");
    let level = xml_attr(xml, "level").unwrap_or("INFO");
    let message = xml
        .find("<![CDATA[")
        .and_then(|start| {
            let content_start = start + "<![CDATA[".len();
            xml[content_start..]
                .find("]]>")
                .map(|len| xml[content_start..content_start + len].trim_end())
        })
        .unwrap_or("");

    // The event's own timestamp, not when the buffered block finished arriving over the pipe
    let timestamp = xml_attr(xml, "timestamp")
        .and_then(|raw| raw.parse::<i64>().ok())
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M:%S").to_string());

    match timestamp {
        Some(ts) => format!("[{ts}] [{thread}/{level}]: {message}"),
        None => format!("[{thread}/{level}]: {message}"),
    }
}

fn patch_real_session_args(game_args: &mut [String], real_access_token: &str) {
    for pair in [("--accessToken", real_access_token), ("--userType", "msa")] {
        let (flag, value) = pair;
        if let Some(pos) = game_args.iter().position(|arg| arg == flag) {
            if let Some(slot) = game_args.get_mut(pos + 1) {
                *slot = value.to_string();
            }
        }
    }
}
