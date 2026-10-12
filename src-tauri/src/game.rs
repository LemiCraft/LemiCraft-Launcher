use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};

use portablemc::{base, fabric, moj};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct GameProcess(pub Arc<Mutex<Option<Child>>>);

// Live "game-log" events only reach windows already listening; this backs a logs window opened after a crash
const LOG_BUFFER_CAP: usize = 3000;

#[derive(Default)]
pub struct GameLogBuffer(pub Arc<Mutex<std::collections::VecDeque<String>>>);

#[tauri::command]
pub fn is_game_running(process: tauri::State<GameProcess>) -> bool {
    process.0.lock().map(|guard| guard.is_some()).unwrap_or(false)
}

#[tauri::command]
pub fn get_log_buffer(buffer: tauri::State<GameLogBuffer>) -> Vec<String> {
    buffer.0.lock().map(|guard| guard.iter().cloned().collect()).unwrap_or_default()
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
    crate::discord_rpc::set_idle(&app);
    restore_main_window(&app, &crate::settings::load().on_launch);

    result
}

// Own config (settings, account, ely.by session) always lives here, never in minecraft_dir(). Debug builds use
// a separate folder; release must match the old C# launcher's for old_launcher_profile_path() (auth.rs)
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

        // Skips Mojang's manifest request (portablemc has no timeout: a throttled VPN hangs the launch), but only once both
        // version jsons are on disk; it's also how a missing one gets downloaded, so a fresh install needs it
        let versions_dir = minecraft_dir().join("versions");
        let has_version_json = |name: &str| versions_dir.join(name).join(format!("{name}.json")).is_file();
        let fabric_root = format!("fabric-{}-{}", crate::config::MC_VERSION, crate::config::FABRIC_LOADER);
        if has_version_json(crate::config::MC_VERSION) && has_version_json(&fabric_root) {
            installer.mojang_mut().add_fetch_exclude(moj::FetchExclude::All);
        }

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

// The official site and our mirror answer in this same shape; the mirror is for networks where GitHub and the official host are blocked
#[derive(serde::Deserialize)]
struct AuthlibRelease {
    download_url: String,
    checksums: AuthlibChecksums,
}

#[derive(serde::Deserialize)]
struct AuthlibChecksums {
    sha256: String,
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

// Per request; the whole attempt across all sources is capped by AUTHLIB_BUDGET.
// 25 s because the first request to a freshly restarted mirror makes it fetch the jar from upstream before answering
const META_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
const JAR_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(40);
const AUTHLIB_BUDGET: std::time::Duration = std::time::Duration::from_secs(90);

fn authlib_descriptor_urls() -> Vec<String> {
    vec![
        "https://authlib-injector.yushi.moe/artifact/latest.json".to_string(),
        format!("{}/launcher/authlib-injector", crate::config::API_BASE),
    ]
}

fn authlib_client() -> Result<reqwest::blocking::Client, String> {
    // The connect limit keeps a blocked host (packets dropped, not refused) from stalling the next source.
    // Redirects stay on https: the jar runs as a javaagent, so a downgrade to plain http must not be followed
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("слишком много перенаправлений")
            } else if attempt.url().scheme() != "https" {
                attempt.error("перенаправление на http")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|err| err.to_string())
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}

fn net_error(err: reqwest::Error) -> String {
    if err.is_timeout() {
        "не ответил вовремя".to_string()
    } else if err.is_connect() {
        "не удалось подключиться".to_string()
    } else if err.is_decode() {
        "вернул неожиданный ответ".to_string()
    } else {
        err.without_url().to_string()
    }
}

// Without the status check any rate-limit or block page surfaces as a bare "error decoding response body"
fn http_get(client: &reqwest::blocking::Client, url: &str, timeout: std::time::Duration) -> Result<reqwest::blocking::Response, String> {
    let res = client.get(url).header("User-Agent", "LemiCraft-Launcher").timeout(timeout).send().map_err(net_error)?;
    if res.status().is_success() {
        Ok(res)
    } else {
        Err(format!("ответил кодом {}", res.status().as_u16()))
    }
}

// A real jar is a zip with its directory at the end, so a truncated file fails here even though it still starts with "PK"
fn is_valid_jar(bytes: &[u8]) -> bool {
    bytes.len() > 4 && &bytes[..2] == b"PK" && zip::ZipArchive::new(std::io::Cursor::new(bytes)).is_ok()
}

fn sha256_hex_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    // Formatted per byte: sha2 0.11's output type doesn't implement LowerHex
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn verify_jar(bytes: &[u8], sha256: Option<&str>) -> Result<(), String> {
    if !is_valid_jar(bytes) {
        return Err("файл повреждён (это не jar-архив)".to_string());
    }
    if let Some(expected) = sha256 {
        if !sha256_hex_of(bytes).eq_ignore_ascii_case(expected.trim()) {
            return Err("контрольная сумма не совпала".to_string());
        }
    }
    Ok(())
}

// A relative link in the answer resolves against the address it came from; the file itself must come over https
fn resolve_jar_url(descriptor_url: &str, download_url: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(descriptor_url)
        .and_then(|base| base.join(download_url))
        .map_err(|err| err.to_string())?;
    if url.scheme() != "https" {
        return Err("ссылка на файл без https".to_string());
    }
    Ok(url)
}

fn download_from_descriptor(client: &reqwest::blocking::Client, descriptor_url: &str) -> Result<Vec<u8>, String> {
    let release: AuthlibRelease = http_get(client, descriptor_url, META_TIMEOUT)?.json().map_err(net_error)?;
    let jar_url = resolve_jar_url(descriptor_url, &release.download_url)?;
    let bytes = http_get(client, jar_url.as_str(), JAR_TIMEOUT)?.bytes().map_err(net_error)?;
    verify_jar(&bytes, Some(&release.checksums.sha256))?;
    Ok(bytes.to_vec())
}

// Ok with the file, or Err with one line per source that failed (or was skipped because the time budget ran out)
fn download_from_sources(
    client: &reqwest::blocking::Client,
    descriptor_urls: &[String],
    deadline: std::time::Instant,
) -> Result<Vec<u8>, Vec<String>> {
    let mut failures = Vec::new();
    for url in descriptor_urls {
        if std::time::Instant::now() >= deadline {
            failures.push(format!("{}: не хватило времени", host_of(url)));
            continue;
        }
        match download_from_descriptor(client, url) {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                log::warn!("authlib-injector from {url}: {err}");
                failures.push(format!("{}: {err}", host_of(url)));
            }
        }
    }
    Err(failures)
}

fn download_from_github(client: &reqwest::blocking::Client) -> Result<Vec<u8>, String> {
    let release: GhRelease = http_get(
        client,
        "https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest",
        META_TIMEOUT,
    )?
    .json()
    .map_err(net_error)?;

    let short_tag = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    let direct_url = format!(
        "https://github.com/yushijinhun/authlib-injector/releases/download/{}/authlib-injector-{short_tag}.jar",
        release.tag_name
    );

    let bytes = match http_get(client, &direct_url, JAR_TIMEOUT).and_then(|res| res.bytes().map_err(net_error)) {
        Ok(bytes) => bytes,
        Err(_) => {
            let asset = release
                .assets
                .iter()
                .find(|a| a.name.contains("authlib-injector") && a.name.ends_with(".jar"))
                .ok_or("в последнем релизе нет authlib-injector.jar")?;
            http_get(client, &asset.browser_download_url, JAR_TIMEOUT)?.bytes().map_err(net_error)?
        }
    };

    verify_jar(&bytes, None)?;
    Ok(bytes.to_vec())
}

// Written aside and renamed so an interrupted write never leaves a half-written jar behind
fn save_jar(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let partial = path.with_extension("jar.part");
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()
    };
    write().map_err(|err| format!("не удалось сохранить authlib-injector.jar: {err}"))?;

    // Antivirus on Windows often keeps a freshly written file open for a moment, which fails the rename
    let mut last_err = None;
    for _ in 0..5 {
        match std::fs::rename(&partial, path) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
    let _ = std::fs::remove_file(&partial);
    Err(format!("не удалось сохранить authlib-injector.jar: {}", last_err.map(|err| err.to_string()).unwrap_or_default()))
}

fn ensure_authlib_injector() -> Result<PathBuf, String> {
    let path = authlib_injector_path();
    if path.exists() {
        if std::fs::read(&path).map(|bytes| is_valid_jar(&bytes)).unwrap_or(false) {
            return Ok(path);
        }
        let _ = std::fs::remove_file(&path);
    }

    let client = authlib_client()?;
    let deadline = std::time::Instant::now() + AUTHLIB_BUDGET;

    let bytes = match download_from_sources(&client, &authlib_descriptor_urls(), deadline) {
        Ok(bytes) => bytes,
        Err(mut failures) => {
            let github = if std::time::Instant::now() < deadline {
                download_from_github(&client)
            } else {
                Err("не хватило времени".to_string())
            };
            match github {
                Ok(bytes) => bytes,
                Err(err) => {
                    log::warn!("authlib-injector from github.com: {err}");
                    failures.push(format!("github.com: {err}"));
                    return Err(format!(
                        "Не удалось скачать authlib-injector, он нужен для входа через Ely.by. Проверьте подключение к интернету или положите файл authlib-injector.jar в папку {}. Подробности: {}",
                        game_dir().display(),
                        failures.join("; ")
                    ));
                }
            }
        }
    };

    std::fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    save_jar(&path, &bytes)?;
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

    #[cfg(unix)]
    if on_launch == "close" {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        let _child = command.spawn().map_err(|err| err.to_string())?;
        let _ = app.emit("game-started", ());
        let app_handle = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            app_handle.exit(0);
        });
        return Ok(());
    }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|err| err.to_string())?;

    let log_buffer = app.state::<GameLogBuffer>().0.clone();
    if let Ok(mut guard) = log_buffer.lock() {
        guard.clear();
    }

    if let Some(stdout) = child.stdout.take() {
        let app = app.clone();
        let log_buffer = log_buffer.clone();
        std::thread::spawn(move || {
            let mut filter = XmlLogFilter::new();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(cleaned) = filter.feed(&line) {
                    push_to_buffer(&log_buffer, &cleaned);
                    let _ = app.emit("game-log", cleaned);
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        let log_buffer = log_buffer.clone();
        std::thread::spawn(move || {
            let mut filter = XmlLogFilter::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Some(cleaned) = filter.feed(&line) {
                    push_to_buffer(&log_buffer, &cleaned);
                    let _ = app.emit("game-log", cleaned);
                }
            }
        });
    }

    *process.lock().map_err(|_| "Не удалось сохранить процесс игры".to_string())? = Some(child);
    let _ = app.emit("game-started", ());
    crate::discord_rpc::set_playing(app);

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
                    crate::discord_rpc::set_idle(&app);
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
                    crate::discord_rpc::set_idle(&app);
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

fn push_to_buffer(buffer: &Arc<Mutex<std::collections::VecDeque<String>>>, line: &str) {
    if let Ok(mut guard) = buffer.lock() {
        if guard.len() >= LOG_BUFFER_CAP {
            guard.pop_front();
        }
        guard.push_back(line.to_string());
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

#[cfg(test)]
mod authlib_tests {
    use super::*;
    use std::io::{Read, Write};

    fn far_deadline() -> std::time::Instant {
        std::time::Instant::now() + std::time::Duration::from_secs(60)
    }

    // Smallest archive that passes the same check as a real jar
    fn tiny_jar() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer.start_file("META-INF/MANIFEST.MF", zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(b"Manifest-Version: 1.0\n").unwrap();
        writer.finish().unwrap().into_inner()
    }

    // One-shot HTTP server on a free local port: answers the first request with the given raw response
    fn serve_raw(response: String) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest.json", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        url
    }

    fn serve_once(status: &str, body: &str) -> String {
        serve_raw(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()))
    }

    #[test]
    fn jar_check_needs_a_real_zip_and_a_matching_hash() {
        let jar = tiny_jar();
        let hash = sha256_hex_of(&jar);
        assert!(verify_jar(&jar, Some(&hash)).is_ok());
        assert!(verify_jar(&jar, Some(&hash.to_uppercase())).is_ok());
        assert!(verify_jar(&jar, None).is_ok());
        assert!(verify_jar(&jar, Some(&"0".repeat(64))).is_err());
        assert!(verify_jar(&jar[..jar.len() - 10], None).is_err(), "a truncated jar still starts with PK");
        assert!(verify_jar(b"PK\x03\x04 not a real archive", None).is_err());
        assert!(verify_jar(b"<html>blocked</html>", None).is_err());
        assert!(verify_jar(b"PK", None).is_err());
    }

    #[test]
    fn jar_link_is_resolved_and_must_be_https() {
        let page = "https://lemicraft.ru/api/launcher/authlib-injector";
        assert_eq!(resolve_jar_url(page, "/api/launcher/authlib-injector/jar?v=ab12").unwrap().as_str(), "https://lemicraft.ru/api/launcher/authlib-injector/jar?v=ab12");
        assert_eq!(resolve_jar_url(page, "https://authlib-injector.yushi.moe/artifact/56/a.jar").unwrap().as_str(), "https://authlib-injector.yushi.moe/artifact/56/a.jar");
        assert!(resolve_jar_url(page, "http://authlib-injector.yushi.moe/a.jar").is_err());
        assert!(resolve_jar_url("http://127.0.0.1:1/latest.json", "/jar").is_err(), "a relative link on an http page stays http");
    }

    #[test]
    fn saving_replaces_an_existing_file_and_leaves_no_part_file() {
        let dir = std::env::temp_dir().join(format!("lc-authlib-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("authlib-injector.jar");
        std::fs::write(&path, b"old").unwrap();
        save_jar(&path, &tiny_jar()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), tiny_jar());
        assert!(!path.with_extension("jar.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The reported bug: a rate-limit answer used to surface as "error decoding response body"
    #[test]
    fn rate_limit_answer_names_the_status_code() {
        let url = serve_once("403 Forbidden", r#"{"message":"API rate limit exceeded"}"#);
        let err = download_from_descriptor(&authlib_client().unwrap(), &url).unwrap_err();
        assert_eq!(err, "ответил кодом 403");
    }

    #[test]
    fn block_page_is_reported_as_unexpected_answer() {
        let url = serve_once("200 OK", "<html>blocked</html>");
        let err = download_from_descriptor(&authlib_client().unwrap(), &url).unwrap_err();
        assert_eq!(err, "вернул неожиданный ответ");
    }

    #[test]
    fn redirect_to_plain_http_is_not_followed() {
        let target = serve_once("200 OK", "{}");
        let url = serve_raw(format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
        let err = download_from_descriptor(&authlib_client().unwrap(), &url).unwrap_err();
        assert_ne!(err, "вернул неожиданный ответ", "the plain http target would have answered 200 with a body");
        assert!(!err.is_empty());
    }

    #[test]
    fn every_failed_source_is_reported() {
        let urls = vec!["http://127.0.0.1:9/a.json".to_string(), "http://127.0.0.1:9/b.json".to_string()];
        let failures = download_from_sources(&authlib_client().unwrap(), &urls, far_deadline()).unwrap_err();
        assert_eq!(failures.len(), 2);
        assert!(failures[0].starts_with("127.0.0.1: "));
    }

    #[test]
    fn sources_after_the_deadline_are_skipped() {
        let urls = vec!["http://127.0.0.1:9/a.json".to_string()];
        let failures = download_from_sources(&authlib_client().unwrap(), &urls, std::time::Instant::now()).unwrap_err();
        assert_eq!(failures, vec!["127.0.0.1: не хватило времени".to_string()]);
    }

    // The tests below use the internet: cargo test -- --ignored
    #[test]
    #[ignore = "needs network"]
    fn official_source_gives_a_verified_jar() {
        let bytes = download_from_sources(&authlib_client().unwrap(), &authlib_descriptor_urls()[..1], far_deadline()).expect("official source");
        assert!(is_valid_jar(&bytes));
    }

    // Passes only once the mirror is deployed on the site
    #[test]
    #[ignore = "needs network and the deployed mirror"]
    fn our_mirror_gives_a_verified_jar() {
        let bytes = download_from_sources(&authlib_client().unwrap(), &authlib_descriptor_urls()[1..], far_deadline()).expect("mirror");
        assert!(is_valid_jar(&bytes));
    }

    #[test]
    #[ignore = "needs network"]
    fn dead_first_source_falls_through_to_the_next() {
        let urls = vec!["http://127.0.0.1:9/latest.json".to_string(), authlib_descriptor_urls().remove(0)];
        let bytes = download_from_sources(&authlib_client().unwrap(), &urls, far_deadline()).expect("second source");
        assert!(is_valid_jar(&bytes));
    }

    #[test]
    #[ignore = "needs network"]
    fn github_fallback_still_works() {
        let bytes = download_from_github(&authlib_client().unwrap()).expect("github");
        assert!(is_valid_jar(&bytes));
    }
}
