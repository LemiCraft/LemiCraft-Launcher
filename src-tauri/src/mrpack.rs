// Import of a Modrinth modpack (.mrpack): a zip with modrinth.index.json (the files to download, with hashes),
// plus overrides/ and client-overrides/ folders. Everything is staged first and moved into place only when all
// of it is downloaded and checked, so a failed import leaves nothing behind.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::game::minecraft_dir;
use crate::mods::{load_import_bundle_state, save_import_bundle_state, ImportBundleState};

// Only these folders of the game directory are touched: not options.txt, not servers.dat (the launcher keeps
// the LemiCraft entry there), nothing outside the game directory
const ALLOWED_DIRS: [&str; 4] = ["mods", "resourcepacks", "shaderpacks", "config"];
// The hosts the Modrinth format allows a file to come from
const ALLOWED_HOSTS: [&str; 4] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

const INDEX_NAME: &str = "modrinth.index.json";
const STAGING_DIR: &str = ".mrpack-staging";
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FILES: usize = 2000;
const MAX_FILE_BYTES: u64 = 300 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 20_000;
const MAX_MOD_JSON_BYTES: u64 = 1024 * 1024;
const PARALLEL_DOWNLOADS: usize = 4;
const DOWNLOAD_ATTEMPTS: u32 = 2;

#[derive(Deserialize)]
struct Index {
    #[serde(rename = "formatVersion")]
    format_version: u32,
    game: String,
    #[serde(rename = "versionId", default)]
    version_id: String,
    name: String,
    #[serde(default)]
    summary: Option<String>,
    files: Vec<IndexFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize, Clone)]
struct IndexFile {
    path: String,
    hashes: Hashes,
    #[serde(default)]
    env: Option<Env>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
}

#[derive(Deserialize, Clone)]
struct Hashes {
    sha512: String,
}

#[derive(Deserialize, Clone)]
struct Env {
    #[serde(default)]
    client: Option<String>,
}

static IMPORT_BUSY: AtomicBool = AtomicBool::new(false);

/// Only one command at a time works on the import slot (the staging folder, incoming.mrpack and the state file):
/// two at once would delete each other's staging, or the second would overwrite what the first records.
pub(crate) struct ImportLock;

impl ImportLock {
    pub(crate) fn acquire() -> Result<ImportLock, String> {
        if IMPORT_BUSY.swap(true, Ordering::AcqRel) {
            Err("Набор уже обрабатывается, подождите".to_string())
        } else {
            Ok(ImportLock)
        }
    }
}

impl Drop for ImportLock {
    fn drop(&mut self) {
        IMPORT_BUSY.store(false, Ordering::Release);
    }
}

/// A path inside the game directory as the pack gives it, or why it is refused. The result uses "/" and
/// starts with one of the allowed folders.
pub fn safe_rel_path(raw: &str) -> Result<String, String> {
    if raw.is_empty() || raw.contains(['\\', ':', '\0']) {
        return Err("недопустимый путь".to_string());
    }
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty() || *p == "." || *p == "..") {
        return Err("недопустимый путь".to_string());
    }
    if !ALLOWED_DIRS.contains(&parts[0]) {
        return Err("папка не поддерживается".to_string());
    }
    // Windows drops a trailing dot or space and reads CON, NUL and the like as devices
    if parts.iter().any(|p| p.ends_with(['.', ' ']) || is_device_name(p)) {
        return Err("недопустимый путь".to_string());
    }
    Ok(parts.join("/"))
}

fn is_device_name(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or(part).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit())
}

fn url_allowed(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else { return false };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.host_str().is_some_and(|host| ALLOWED_HOSTS.contains(&host))
}

// GitHub serves release files from its own storage hosts
fn redirect_allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(|host| ALLOWED_HOSTS.contains(&host) || host.ends_with(".githubusercontent.com"))
}

fn version_parts(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// Why this pack cannot be played with the launcher's game, if it cannot
fn compat_error(index: &Index, minecraft: &str, loader: &str) -> Option<String> {
    let deps = &index.dependencies;
    for (key, name) in [("forge", "Forge"), ("neoforge", "NeoForge"), ("quilt-loader", "Quilt")] {
        if deps.contains_key(key) {
            return Some(format!("Набор создан для {name}, а лаунчер работает с Fabric"));
        }
    }
    let Some(pack_loader) = deps.get("fabric-loader") else {
        return Some("В наборе не указан Fabric: подходят только наборы для Fabric".to_string());
    };
    match deps.get("minecraft") {
        Some(version) if version == minecraft => {}
        Some(version) => return Some(format!("Набор собран для Minecraft {version}, а в лаунчере {minecraft}")),
        None => return Some("В наборе не указана версия Minecraft".to_string()),
    }
    // A pack that needs a newer loader than the one installed does not start
    if let (Some(need), Some(have)) = (version_parts(pack_loader), version_parts(loader)) {
        if need > have {
            return Some(format!("Набору нужен Fabric Loader {pack_loader}, а в лаунчере {loader}"));
        }
    }
    None
}

struct PlannedFile {
    rel: String,
    file: IndexFile,
    urls: Vec<String>,
}

#[derive(Default)]
struct Plan {
    files: Vec<PlannedFile>,
    client_unsupported: usize,
    rejected: Vec<String>,
    total_size: u64,
}

fn plan(index: &Index) -> Result<Plan, String> {
    if index.format_version != 1 {
        return Err("Эта версия формата набора не поддерживается".to_string());
    }
    if index.game != "minecraft" {
        return Err("Набор не для Minecraft".to_string());
    }
    if index.files.len() > MAX_FILES {
        return Err("В наборе слишком много файлов".to_string());
    }
    let mut plan = Plan::default();
    let mut seen = HashSet::new();
    for file in &index.files {
        if file.env.as_ref().and_then(|env| env.client.as_deref()) == Some("unsupported") {
            plan.client_unsupported += 1;
            continue;
        }
        let Ok(rel) = safe_rel_path(&file.path) else {
            plan.rejected.push(file.path.clone());
            continue;
        };
        let urls: Vec<String> = file.downloads.iter().filter(|url| url_allowed(url)).cloned().collect();
        // the size is required by the format, and without it the total could not be kept; paths that differ only in case are one file on Windows
        if urls.is_empty() || file.file_size == 0 || file.file_size > MAX_FILE_BYTES || !seen.insert(rel.to_lowercase()) {
            plan.rejected.push(file.path.clone());
            continue;
        }
        plan.total_size += file.file_size;
        plan.files.push(PlannedFile { rel, file: file.clone(), urls });
    }
    if plan.total_size > MAX_TOTAL_BYTES {
        return Err("Набор слишком большой".to_string());
    }
    Ok(plan)
}

fn read_index(archive: &mut zip::ZipArchive<File>) -> Result<Index, String> {
    let entry = archive.by_name(INDEX_NAME).map_err(|_| "Это не набор Modrinth: в архиве нет modrinth.index.json".to_string())?;
    let mut raw = Vec::new();
    entry.take(MAX_INDEX_BYTES + 1).read_to_end(&mut raw).map_err(|err| err.to_string())?;
    if raw.len() as u64 > MAX_INDEX_BYTES {
        return Err("Описание набора слишком велико".to_string());
    }
    serde_json::from_slice(&raw).map_err(|err| format!("Описание набора повреждено: {err}"))
}

fn open_pack(path: &Path) -> Result<zip::ZipArchive<File>, String> {
    let file = File::open(path).map_err(|err| format!("Не удалось открыть файл: {err}"))?;
    let archive = zip::ZipArchive::new(file).map_err(|_| "Файл не похож на набор Modrinth (.mrpack)".to_string())?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err("Архив содержит слишком много файлов".to_string());
    }
    Ok(archive)
}

/// Overrides of the pack that would be extracted: ("mods/x.jar", entry index). client-overrides come after
/// overrides, so they replace the same path.
fn override_entries(archive: &mut zip::ZipArchive<File>) -> Vec<(String, usize)> {
    let mut found: Vec<(String, usize)> = Vec::new();
    for prefix in ["overrides", "client-overrides"] {
        for i in 0..archive.len() {
            let Ok(entry) = archive.by_index(i) else { continue };
            if entry.is_dir() {
                continue;
            }
            let Some(enclosed) = entry.enclosed_name() else { continue };
            let Ok(rest) = enclosed.strip_prefix(prefix) else { continue };
            let parts: Vec<String> = rest
                .components()
                .filter_map(|c| match c {
                    Component::Normal(part) => part.to_str().map(str::to_string),
                    _ => None,
                })
                .collect();
            let Ok(rel) = safe_rel_path(&parts.join("/")) else { continue };
            match found.iter_mut().find(|(existing, _)| *existing == rel) {
                Some(slot) => slot.1 = i,
                None => found.push((rel, i)),
            }
        }
    }
    found
}

fn sha512_hex_of_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha512};
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha512::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|err| err.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    // Formatted per byte: sha2 0.11's output type does not implement LowerHex
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The Fabric id of a mod jar. Fabric refuses to start with two jars of one id, so duplicates are found by it, not by file name.
fn jar_mod_id(path: &Path) -> Option<String> {
    let mut archive = zip::ZipArchive::new(File::open(path).ok()?).ok()?;
    let entry = archive.by_name("fabric.mod.json").ok()?;
    let mut raw = Vec::new();
    entry.take(MAX_MOD_JSON_BYTES).read_to_end(&mut raw).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    json.get("id")?.as_str().map(str::to_string)
}

fn installed_mod_ids(mods_dir: &Path, ignore: &HashSet<String>) -> HashSet<String> {
    let Ok(read_dir) = std::fs::read_dir(mods_dir) else { return HashSet::new() };
    read_dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_string();
            if path.extension()?.to_str()? != "jar" || ignore.contains(&format!("mods/{name}")) {
                return None;
            }
            jar_mod_id(&path)
        })
        .collect()
}

trait Fetch: Sync {
    /// Writes the body of `url` to `dest`, at most `max_bytes` of it
    fn fetch(&self, url: &str, dest: &Path, max_bytes: u64) -> Result<(), String>;
}

struct HttpFetch(reqwest::blocking::Client);

impl HttpFetch {
    fn new() -> Result<Self, String> {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("слишком много перенаправлений")
                } else if redirect_allowed(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.error("перенаправление на недопустимый адрес")
                }
            }))
            .build()
            .map(HttpFetch)
            .map_err(|err| err.to_string())
    }
}

impl Fetch for HttpFetch {
    fn fetch(&self, url: &str, dest: &Path, max_bytes: u64) -> Result<(), String> {
        // The time allowed grows with the size, so a big file on a slow line is not cut at a fixed mark (about
        // 800 kbit/s at the least), and a stalled one still ends
        let allowed = Duration::from_secs((max_bytes / 100_000).clamp(60, 3600));
        let res = self.0.get(url).timeout(allowed).send().map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Err(format!("ответ {}", res.status()));
        }
        if res.content_length().is_some_and(|len| len > max_bytes) {
            return Err("файл больше заявленного".to_string());
        }
        let mut out = File::create(dest).map_err(|err| err.to_string())?;
        let copied = std::io::copy(&mut res.take(max_bytes + 1), &mut out).map_err(|err| err.to_string())?;
        if copied > max_bytes {
            return Err("файл больше заявленного".to_string());
        }
        out.flush().map_err(|err| err.to_string())
    }
}

fn file_label(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn download_one(item: &PlannedFile, staging: &Path, fetcher: &dyn Fetch) -> Result<(), String> {
    let dest = staging.join(&item.rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let max = if item.file.file_size > 0 { item.file.file_size } else { MAX_FILE_BYTES };
    let mut last_error = String::from("нет адреса для загрузки");
    for url in &item.urls {
        for attempt in 1..=DOWNLOAD_ATTEMPTS {
            if attempt > 1 {
                std::thread::sleep(Duration::from_millis(800));
            }
            let result = fetcher.fetch(url, &dest, max).and_then(|()| {
                let size = std::fs::metadata(&dest).map_err(|err| err.to_string())?.len();
                if item.file.file_size > 0 && size != item.file.file_size {
                    return Err("размер файла не совпал".to_string());
                }
                if !sha512_hex_of_file(&dest)?.eq_ignore_ascii_case(item.file.hashes.sha512.trim()) {
                    return Err("контрольная сумма не совпала".to_string());
                }
                Ok(())
            });
            match result {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_error = err;
                    let _ = std::fs::remove_file(&dest);
                }
            }
        }
    }
    Err(format!("{}: {last_error}", file_label(&item.rel)))
}

fn download_all(
    items: &[PlannedFile],
    staging: &Path,
    fetcher: &dyn Fetch,
    progress: &(dyn Fn(usize, usize, &str) + Sync),
) -> Result<(), String> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failure: Mutex<Option<String>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL_DOWNLOADS.min(items.len()).max(1) {
            scope.spawn(|| loop {
                if failure.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                    return;
                }
                let Some(item) = items.get(next.fetch_add(1, Ordering::SeqCst)) else { return };
                match download_one(item, staging, fetcher) {
                    Ok(()) => {
                        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                        progress(n, items.len(), file_label(&item.rel));
                    }
                    Err(err) => {
                        *failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(err);
                        return;
                    }
                }
            });
        }
    });
    match failure.into_inner().unwrap_or_else(|e| e.into_inner()) {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Removes the staging folder however the import ends
struct StagingGuard(PathBuf);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Outcome {
    /// Paths relative to the game directory, "/" separated: everything this import put there
    pub files: Vec<String>,
    /// The Fabric ids of the installed mods, prefixed so they never match an id of the site's catalog
    pub mod_ids: Vec<String>,
    pub mods: usize,
    /// Files that were already there and were left as they are
    pub kept_existing: usize,
    /// Mods skipped because a jar with the same Fabric id is installed
    pub already_installed: Vec<String>,
    /// Entries of the pack that were refused (unsafe path, host, size)
    pub rejected: usize,
}

fn is_jar_in_mods(rel: &str) -> bool {
    rel.starts_with("mods/") && rel.ends_with(".jar")
}

fn move_into_place(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // The staging folder can sit on another disk than a moved game folder
    if let Err(err) = std::fs::copy(from, to) {
        let _ = std::fs::remove_file(to);
        return Err(err.to_string());
    }
    let _ = std::fs::remove_file(from);
    Ok(())
}

/// Installs the pack into `minecraft_dir`. `previous` are the files of the import it replaces (relative, "/").
fn install_into(
    pack: &Path,
    minecraft_dir: &Path,
    minecraft: &str,
    loader: &str,
    fetcher: &dyn Fetch,
    previous: &HashSet<String>,
    progress: &(dyn Fn(usize, usize, &str) + Sync),
) -> Result<Outcome, String> {
    let mut archive = open_pack(pack)?;
    let index = read_index(&mut archive)?;
    if let Some(reason) = compat_error(&index, minecraft, loader) {
        return Err(reason);
    }
    let plan = plan(&index)?;

    let staging = minecraft_dir.join(STAGING_DIR);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|err| err.to_string())?;
    let _guard = StagingGuard(staging.clone());

    download_all(&plan.files, &staging, fetcher, progress)?;

    let mut outcome = Outcome { rejected: plan.rejected.len(), ..Outcome::default() };
    let mut mod_ids = installed_mod_ids(&minecraft_dir.join("mods"), previous);
    // (relative path, staged file) of what goes into the game directory
    let mut to_move: Vec<(String, PathBuf)> = Vec::new();

    let mut consider = |rel: String, staged: PathBuf, outcome: &mut Outcome| {
        let dest = minecraft_dir.join(&rel);
        if dest.exists() && !previous.contains(&rel) {
            outcome.kept_existing += 1;
            return;
        }
        if is_jar_in_mods(&rel) {
            let id = jar_mod_id(&staged);
            if let Some(id) = &id {
                if !mod_ids.insert(id.clone()) {
                    outcome.already_installed.push(file_label(&rel).to_string());
                    return;
                }
            }
            outcome.mods += 1;
            outcome.mod_ids.push(format!("mrpack:{}", id.unwrap_or_else(|| file_label(&rel).trim_end_matches(".jar").to_string())));
        }
        to_move.push((rel, staged));
    };

    let staged_from_files: HashSet<String> = plan.files.iter().map(|f| f.rel.clone()).collect();
    for item in &plan.files {
        consider(item.rel.clone(), staging.join(&item.rel), &mut outcome);
    }

    // Overrides: configs, resource packs and the like that come inside the archive. A file the pack lists by
    // download wins over an override of the same path; what is already in the game folder stays.
    let mut written: u64 = 0;
    for (rel, entry_index) in override_entries(&mut archive) {
        if staged_from_files.contains(&rel) {
            continue;
        }
        let staged = staging.join(&rel);
        if let Some(parent) = staged.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut entry = archive.by_index(entry_index).map_err(|err| err.to_string())?;
        let mut out = File::create(&staged).map_err(|err| err.to_string())?;
        // Caps the bytes really copied, whatever size the entry claims
        let copied = std::io::copy(&mut (&mut entry).take(MAX_TOTAL_BYTES.saturating_sub(written) + 1), &mut out)
            .map_err(|err| err.to_string())?;
        written += copied;
        if written > MAX_TOTAL_BYTES {
            return Err("Набор слишком большой после распаковки".to_string());
        }
        consider(rel, staged, &mut outcome);
    }

    if to_move.is_empty() {
        return Err("В наборе нечего устанавливать: всё уже есть или не подходит".to_string());
    }

    // Moved in one go; if one move fails the files of this import placed so far are taken back out
    let mut placed: Vec<PathBuf> = Vec::new();
    for (rel, staged) in &to_move {
        let dest = minecraft_dir.join(rel);
        if let Err(err) = move_into_place(staged, &dest) {
            for path in &placed {
                let _ = std::fs::remove_file(path);
            }
            return Err(format!("Не удалось установить {}: {err}", file_label(rel)));
        }
        placed.push(dest);
        outcome.files.push(rel.clone());
    }

    // Files of the import this one replaces and that are not part of it go now
    for old in previous {
        if !outcome.files.contains(old) && safe_rel_path(old).is_ok() {
            let _ = std::fs::remove_file(minecraft_dir.join(old));
        }
    }
    Ok(outcome)
}

#[derive(Serialize)]
pub struct MrpackPreview {
    name: String,
    version: String,
    summary: String,
    mods: usize,
    resourcepacks: usize,
    shaderpacks: usize,
    /// Files that come inside the archive (configs and the like)
    overrides: usize,
    total_size: u64,
    /// Entries that will not be installed: a path or a host the launcher does not accept
    rejected: Vec<String>,
    /// Mods the pack only wants on a server
    client_unsupported: usize,
    /// Why the pack cannot be installed, when it cannot
    incompatible: Option<String>,
    /// An import already installed is replaced by this one
    replaces_import: bool,
}

fn preview_of(pack: &Path, minecraft: &str, loader: &str, replaces_import: bool) -> Result<MrpackPreview, String> {
    let mut archive = open_pack(pack)?;
    let index = read_index(&mut archive)?;
    let incompatible = compat_error(&index, minecraft, loader);
    let plan = plan(&index)?;
    let count = |dir: &str| plan.files.iter().filter(|f| f.rel.starts_with(&format!("{dir}/"))).count();
    let overrides = override_entries(&mut archive)
        .into_iter()
        .filter(|(rel, _)| !plan.files.iter().any(|f| &f.rel == rel))
        .count();
    Ok(MrpackPreview {
        name: index.name.clone(),
        version: index.version_id.clone(),
        summary: index.summary.clone().unwrap_or_default(),
        mods: count("mods"),
        resourcepacks: count("resourcepacks"),
        shaderpacks: count("shaderpacks"),
        overrides,
        total_size: plan.total_size,
        rejected: plan.rejected,
        client_unsupported: plan.client_unsupported,
        incompatible,
        replaces_import,
    })
}

fn previous_files() -> HashSet<String> {
    load_import_bundle_state()
        .map(|state| state.files.into_iter().map(|f| f.replace('\\', "/")).collect())
        .unwrap_or_default()
}

// A pack dropped on the window comes as bytes, without a path: it is written here and then used like a chosen file
const MAX_DROPPED_PACK_BYTES: usize = 600 * 1024 * 1024;

fn staged_pack_path() -> PathBuf {
    crate::game::game_dir().join("incoming.mrpack")
}

#[tauri::command]
pub async fn stage_mrpack(request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("Файл не получен".to_string());
    };
    if bytes.len() > MAX_DROPPED_PACK_BYTES {
        return Err("Файл слишком большой".to_string());
    }
    let lock = ImportLock::acquire()?;
    let bytes = bytes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = lock;
        let path = staged_pack_path();
        std::fs::create_dir_all(crate::game::game_dir()).map_err(|err| err.to_string())?;
        std::fs::write(&path, bytes).map_err(|err| format!("Не удалось сохранить файл: {err}"))?;
        Ok(path.to_string_lossy().to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn preview_mrpack(path: String) -> Result<MrpackPreview, String> {
    let lock = ImportLock::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = lock;
        preview_of(
            Path::new(&path),
            crate::config::MC_VERSION,
            crate::config::FABRIC_LOADER,
            load_import_bundle_state().is_some(),
        )
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn apply_mrpack(app: AppHandle, path: String) -> Result<Outcome, String> {
    let lock = ImportLock::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = lock;
        let fetcher = HttpFetch::new()?;
        let previous = previous_files();
        let progress = |current: usize, total: usize, name: &str| {
            let _ = app.emit("mods-progress", serde_json::json!({ "stage": "mrpack", "current": current, "total": total, "id": name }));
        };
        let outcome = install_into(
            Path::new(&path),
            &minecraft_dir(),
            crate::config::MC_VERSION,
            crate::config::FABRIC_LOADER,
            &fetcher,
            &previous,
            &progress,
        )?;
        if outcome.files.is_empty() {
            return Err("В наборе нечего устанавливать: всё уже есть или не подходит".to_string());
        }
        save_import_bundle_state(&ImportBundleState { files: outcome.files.clone(), mod_ids: outcome.mod_ids.clone() })?;
        // a dropped pack was only kept for this; a failed import keeps it, so that it can be tried again
        if Path::new(&path) == staged_pack_path() {
            let _ = std::fs::remove_file(&path);
        }
        Ok(outcome)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_paths_in_the_allowed_folders_are_accepted() {
        assert_eq!(safe_rel_path("mods/sodium.jar").unwrap(), "mods/sodium.jar");
        assert_eq!(safe_rel_path("config/sodium/options.json").unwrap(), "config/sodium/options.json");
        for bad in [
            "",
            "mods",
            "../mods/x.jar",
            "mods/../../x.jar",
            "mods/./x.jar",
            "/mods/x.jar",
            "mods//x.jar",
            "mods\\x.jar",
            "C:/mods/x.jar",
            "mods/x:y.jar",
            "mods/x\0.jar",
            "options.txt",
            "servers.dat",
            "saves/world/level.dat",
            "versions/x/x.jar",
        ] {
            assert!(safe_rel_path(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn files_come_only_from_https_hosts_the_format_allows() {
        assert!(url_allowed("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"));
        assert!(url_allowed("https://github.com/o/r/releases/download/v1/a.jar"));
        assert!(url_allowed("https://raw.githubusercontent.com/o/r/main/a.jar"));
        for bad in [
            "http://cdn.modrinth.com/a.jar",
            "https://evil.example/a.jar",
            "https://cdn.modrinth.com.evil.example/a.jar",
            "https://user:pw@cdn.modrinth.com/a.jar",
            "https://cdn.modrinth.com:8443/a.jar",
            "ftp://cdn.modrinth.com/a.jar",
            "file:///etc/passwd",
            "not a url",
        ] {
            assert!(!url_allowed(bad), "{bad:?} must be refused");
        }
    }

    #[test]
    fn a_redirect_may_go_to_github_storage_but_not_to_plain_http_or_other_hosts() {
        let parse = |s: &str| reqwest::Url::parse(s).unwrap();
        assert!(redirect_allowed(&parse("https://release-assets.githubusercontent.com/x")));
        assert!(redirect_allowed(&parse("https://cdn.modrinth.com/x")));
        assert!(!redirect_allowed(&parse("http://cdn.modrinth.com/x")));
        assert!(!redirect_allowed(&parse("https://githubusercontent.com.evil.example/x")));
        assert!(!redirect_allowed(&parse("https://evil.example/x")));
    }

    fn index_json(deps: &str, files: &str) -> String {
        format!(r#"{{"formatVersion":1,"game":"minecraft","versionId":"1.0","name":"Pack","files":[{files}],"dependencies":{{{deps}}}}}"#)
    }

    fn parse(json: &str) -> Index {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_pack_has_to_match_the_game_and_the_loader() {
        let ok = parse(&index_json(r#""minecraft":"26.2","fabric-loader":"0.19.5""#, ""));
        assert_eq!(compat_error(&ok, "26.2", "0.19.5"), None);
        // an older loader requirement is fine, a newer one is not
        assert_eq!(compat_error(&parse(&index_json(r#""minecraft":"26.2","fabric-loader":"0.16.9""#, "")), "26.2", "0.19.5"), None);
        assert!(compat_error(&parse(&index_json(r#""minecraft":"26.2","fabric-loader":"0.20.0""#, "")), "26.2", "0.19.5").unwrap().contains("0.20.0"));
        assert!(compat_error(&parse(&index_json(r#""minecraft":"1.21.1","fabric-loader":"0.16.9""#, "")), "26.2", "0.19.5").unwrap().contains("1.21.1"));
        assert!(compat_error(&parse(&index_json(r#""minecraft":"26.2","forge":"52.0.0""#, "")), "26.2", "0.19.5").unwrap().contains("Forge"));
        assert!(compat_error(&parse(&index_json(r#""minecraft":"26.2","neoforge":"21.1.0""#, "")), "26.2", "0.19.5").unwrap().contains("NeoForge"));
        assert!(compat_error(&parse(&index_json(r#""minecraft":"26.2","quilt-loader":"0.26.0""#, "")), "26.2", "0.19.5").unwrap().contains("Quilt"));
        assert!(compat_error(&parse(&index_json(r#""minecraft":"26.2""#, "")), "26.2", "0.19.5").is_some());
        assert!(compat_error(&parse(&index_json(r#""fabric-loader":"0.19.5""#, "")), "26.2", "0.19.5").is_some());
    }

    fn file_json(path: &str, url: &str, sha512: &str, size: u64, env: &str) -> String {
        format!(r#"{{"path":"{path}","hashes":{{"sha1":"x","sha512":"{sha512}"}},"downloads":["{url}"],"fileSize":{size}{env}}}"#)
    }

    #[test]
    fn the_plan_skips_server_only_files_and_refuses_unsafe_entries() {
        let files = [
            file_json("mods/a.jar", "https://cdn.modrinth.com/a.jar", "aa", 10, ""),
            file_json("mods/server.jar", "https://cdn.modrinth.com/s.jar", "bb", 10, r#","env":{"client":"unsupported","server":"required"}"#),
            file_json("mods/optional.jar", "https://cdn.modrinth.com/o.jar", "cc", 10, r#","env":{"client":"optional","server":"optional"}"#),
            file_json("../escape.jar", "https://cdn.modrinth.com/e.jar", "dd", 10, ""),
            file_json("mods/from-elsewhere.jar", "https://evil.example/x.jar", "ee", 10, ""),
            file_json("mods/a.jar", "https://cdn.modrinth.com/a2.jar", "ff", 10, ""),
            file_json("mods/huge.jar", "https://cdn.modrinth.com/h.jar", "gg", MAX_FILE_BYTES + 1, ""),
            file_json("options.txt", "https://cdn.modrinth.com/o.txt", "hh", 10, ""),
        ]
        .join(",");
        let plan = plan(&parse(&index_json(r#""minecraft":"26.2","fabric-loader":"0.19.5""#, &files))).unwrap();
        let rels: Vec<&str> = plan.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels, vec!["mods/a.jar", "mods/optional.jar"]);
        assert_eq!(plan.client_unsupported, 1);
        assert_eq!(plan.rejected.len(), 5);
        assert_eq!(plan.total_size, 20);
    }

    #[test]
    fn another_format_or_game_is_refused() {
        let mut index = parse(&index_json("", ""));
        index.format_version = 2;
        assert!(plan(&index).is_err());
        let mut index = parse(&index_json("", ""));
        index.game = "hytale".to_string();
        assert!(plan(&index).is_err());
    }

    // ---- install, against a temporary game folder and a fake download ----

    struct Dirs {
        root: PathBuf,
    }

    impl Dirs {
        fn new(name: &str) -> Dirs {
            let root = std::env::temp_dir().join(format!("lc-mrpack-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("game")).unwrap();
            Dirs { root }
        }

        fn game(&self) -> PathBuf {
            self.root.join("game")
        }

        fn pack(&self) -> PathBuf {
            self.root.join("pack.mrpack")
        }
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn sha512_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha512};
        Sha512::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    // A jar with a fabric.mod.json, built once per use: a zip carries its creation time
    fn jar_with_id(id: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(format!(r#"{{"schemaVersion":1,"id":"{id}","version":"1"}}"#).as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    struct FakeFetch {
        bodies: HashMap<String, Vec<u8>>,
    }

    impl Fetch for FakeFetch {
        fn fetch(&self, url: &str, dest: &Path, _max: u64) -> Result<(), String> {
            let body = self.bodies.get(url).ok_or_else(|| "ответ 404".to_string())?;
            std::fs::write(dest, body).map_err(|err| err.to_string())
        }
    }

    struct Entry {
        path: &'static str,
        body: Vec<u8>,
    }

    /// Writes a pack with the given downloadable files and archive entries (overrides/…)
    fn write_pack(dirs: &Dirs, deps: &str, downloads: &[Entry], archive_entries: &[(&str, &[u8])]) -> FakeFetch {
        let mut bodies = HashMap::new();
        let mut files = Vec::new();
        for entry in downloads {
            let url = format!("https://cdn.modrinth.com/data/{}", entry.path.replace('/', "_"));
            files.push(file_json(entry.path, &url, &sha512_hex(&entry.body), entry.body.len() as u64, ""));
            bodies.insert(url, entry.body.clone());
        }
        let mut writer = zip::ZipWriter::new(File::create(dirs.pack()).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file(INDEX_NAME, options).unwrap();
        writer.write_all(index_json(deps, &files.join(",")).as_bytes()).unwrap();
        for (name, body) in archive_entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
        FakeFetch { bodies }
    }

    const DEPS: &str = r#""minecraft":"26.2","fabric-loader":"0.19.5""#;

    fn install(dirs: &Dirs, fetcher: &FakeFetch, previous: &[&str]) -> Result<Outcome, String> {
        let previous: HashSet<String> = previous.iter().map(|s| s.to_string()).collect();
        install_into(&dirs.pack(), &dirs.game(), "26.2", "0.19.5", fetcher, &previous, &|_, _, _| {})
    }

    #[test]
    fn a_pack_is_installed_with_its_overrides_and_leaves_no_staging_folder() {
        let dirs = Dirs::new("install");
        let fetcher = write_pack(
            &dirs,
            DEPS,
            &[Entry { path: "mods/a.jar", body: jar_with_id("a") }, Entry { path: "resourcepacks/rp.zip", body: b"zip".to_vec() }],
            &[("overrides/config/a.toml", b"x=1"), ("client-overrides/config/b.toml", b"y=2"), ("overrides/servers.dat", b"nope"), ("overrides/options.txt", b"nope"), ("server-overrides/config/s.toml", b"nope")],
        );
        let outcome = install(&dirs, &fetcher, &[]).unwrap();
        let mut files = outcome.files.clone();
        files.sort();
        assert_eq!(files, vec!["config/a.toml", "config/b.toml", "mods/a.jar", "resourcepacks/rp.zip"]);
        assert_eq!((outcome.mods, outcome.mod_ids.clone()), (1, vec!["mrpack:a".to_string()]));
        assert!(dirs.game().join("mods/a.jar").is_file());
        assert_eq!(std::fs::read(dirs.game().join("config/b.toml")).unwrap(), b"y=2");
        assert!(!dirs.game().join("servers.dat").exists() && !dirs.game().join("options.txt").exists());
        assert!(!dirs.game().join("config/s.toml").exists());
        assert!(!dirs.game().join(STAGING_DIR).exists());
    }

    #[test]
    fn a_wrong_hash_stops_everything_and_nothing_stays_in_the_game_folder() {
        let dirs = Dirs::new("hash");
        let mut fetcher = write_pack(&dirs, DEPS, &[Entry { path: "mods/a.jar", body: jar_with_id("a") }, Entry { path: "mods/b.jar", body: jar_with_id("b") }], &[]);
        // the server sends something else than the pack promised for b, of the same length
        let url_b = "https://cdn.modrinth.com/data/mods_b.jar".to_string();
        let tampered = fetcher.bodies.get(&url_b).unwrap().iter().map(|byte| byte ^ 0x55).collect();
        fetcher.bodies.insert(url_b, tampered);
        let err = install(&dirs, &fetcher, &[]).unwrap_err();
        assert!(err.contains("b.jar") && err.contains("контрольная сумма"), "{err}");
        assert!(!dirs.game().join("mods").exists() || std::fs::read_dir(dirs.game().join("mods")).unwrap().next().is_none());
        assert!(!dirs.game().join(STAGING_DIR).exists());
    }

    #[test]
    fn a_file_of_the_wrong_size_is_refused() {
        let dirs = Dirs::new("size");
        let mut fetcher = write_pack(&dirs, DEPS, &[Entry { path: "mods/a.jar", body: jar_with_id("a") }], &[]);
        fetcher.bodies.insert("https://cdn.modrinth.com/data/mods_a.jar".to_string(), b"short".to_vec());
        assert!(install(&dirs, &fetcher, &[]).unwrap_err().contains("размер"));
        assert!(!dirs.game().join("mods/a.jar").exists());
    }

    #[test]
    fn a_file_that_cannot_be_downloaded_stops_the_import() {
        let dirs = Dirs::new("missing");
        let mut fetcher = write_pack(&dirs, DEPS, &[Entry { path: "mods/a.jar", body: jar_with_id("a") }], &[]);
        fetcher.bodies.clear();
        assert!(install(&dirs, &fetcher, &[]).unwrap_err().contains("a.jar"));
        assert!(!dirs.game().join("mods/a.jar").exists());
    }

    #[test]
    fn a_mod_already_installed_under_another_file_name_is_skipped_by_its_fabric_id() {
        let dirs = Dirs::new("dupe");
        std::fs::create_dir_all(dirs.game().join("mods")).unwrap();
        std::fs::write(dirs.game().join("mods/sodium-fabric-0.5.jar"), jar_with_id("sodium")).unwrap();
        let fetcher = write_pack(
            &dirs,
            DEPS,
            &[Entry { path: "mods/sodium-0.6.jar", body: jar_with_id("sodium") }, Entry { path: "mods/lithium.jar", body: jar_with_id("lithium") }, Entry { path: "mods/lithium-copy.jar", body: jar_with_id("lithium") }],
            &[],
        );
        let outcome = install(&dirs, &fetcher, &[]).unwrap();
        assert_eq!(outcome.files, vec!["mods/lithium.jar"]);
        assert_eq!(outcome.already_installed.len(), 2);
        assert!(dirs.game().join("mods/sodium-fabric-0.5.jar").is_file());
        assert!(!dirs.game().join("mods/sodium-0.6.jar").exists());
    }

    #[test]
    fn files_that_are_already_there_are_kept_and_the_users_config_is_not_overwritten() {
        let dirs = Dirs::new("keep");
        std::fs::create_dir_all(dirs.game().join("config")).unwrap();
        std::fs::write(dirs.game().join("config/a.toml"), b"mine").unwrap();
        let fetcher = write_pack(&dirs, DEPS, &[Entry { path: "resourcepacks/rp.zip", body: b"zip".to_vec() }], &[("overrides/config/a.toml", b"theirs")]);
        let outcome = install(&dirs, &fetcher, &[]).unwrap();
        assert_eq!(std::fs::read(dirs.game().join("config/a.toml")).unwrap(), b"mine");
        assert_eq!(outcome.kept_existing, 1);
        assert_eq!(outcome.files, vec!["resourcepacks/rp.zip"]);
    }

    #[test]
    fn a_new_import_takes_the_files_of_the_previous_one_it_replaces() {
        let dirs = Dirs::new("replace");
        std::fs::create_dir_all(dirs.game().join("mods")).unwrap();
        std::fs::write(dirs.game().join("mods/old.jar"), jar_with_id("old")).unwrap();
        std::fs::write(dirs.game().join("mods/shared.jar"), jar_with_id("shared")).unwrap();
        std::fs::write(dirs.game().join("mods/theirs.jar"), jar_with_id("theirs")).unwrap();
        let fetcher = write_pack(&dirs, DEPS, &[Entry { path: "mods/shared.jar", body: jar_with_id("shared") }, Entry { path: "mods/new.jar", body: jar_with_id("new") }], &[]);
        let outcome = install(&dirs, &fetcher, &["mods/old.jar", "mods/shared.jar"]).unwrap();
        let mut files = outcome.files.clone();
        files.sort();
        // shared.jar belongs to the previous import, so it is replaced rather than kept; old.jar goes; theirs.jar is not ours
        assert_eq!(files, vec!["mods/new.jar", "mods/shared.jar"]);
        assert!(!dirs.game().join("mods/old.jar").exists());
        assert!(dirs.game().join("mods/theirs.jar").is_file());
    }

    #[test]
    fn an_import_with_nothing_to_install_leaves_the_previous_one_alone() {
        let dirs = Dirs::new("nothing");
        std::fs::create_dir_all(dirs.game().join("mods")).unwrap();
        std::fs::write(dirs.game().join("mods/old.jar"), jar_with_id("old")).unwrap();
        // the only file of the pack is one the user already has, so nothing would be installed
        std::fs::write(dirs.game().join("mods/theirs.jar"), jar_with_id("theirs")).unwrap();
        let fetcher = write_pack(&dirs, DEPS, &[Entry { path: "mods/theirs.jar", body: jar_with_id("theirs") }], &[]);
        let err = install(&dirs, &fetcher, &["mods/old.jar"]).unwrap_err();
        assert!(err.contains("нечего"), "{err}");
        assert!(dirs.game().join("mods/old.jar").is_file());
    }

    #[test]
    fn the_previous_import_never_takes_files_outside_the_allowed_folders_with_it() {
        let dirs = Dirs::new("options");
        std::fs::write(dirs.game().join("options.txt"), b"mine").unwrap_or_default();
        let fetcher = write_pack(&dirs, DEPS, &[Entry { path: "mods/new.jar", body: jar_with_id("new") }], &[]);
        install(&dirs, &fetcher, &["options.txt"]).unwrap();
        assert!(dirs.game().join("options.txt").is_file());
    }

    #[test]
    fn names_windows_would_read_as_something_else_are_refused() {
        for raw in ["mods/NUL", "mods/nul.jar", "config/con.txt", "mods/x.jar.", "mods/x.jar ", "mods/COM1.jar", "mods/lpt9"] {
            assert!(safe_rel_path(raw).is_err(), "{raw}");
        }
        assert!(safe_rel_path("mods/console.jar").is_ok());
        assert!(safe_rel_path("mods/common.jar").is_ok());
    }

    #[test]
    fn a_pack_for_another_game_version_is_refused_before_anything_is_downloaded() {
        let dirs = Dirs::new("version");
        let fetcher = write_pack(&dirs, r#""minecraft":"1.21.1","fabric-loader":"0.16.9""#, &[Entry { path: "mods/a.jar", body: jar_with_id("a") }], &[]);
        let err = install(&dirs, &fetcher, &[]).unwrap_err();
        assert!(err.contains("1.21.1"), "{err}");
        assert!(!dirs.game().join(STAGING_DIR).exists());
    }

    #[test]
    fn things_that_are_not_packs_are_refused_with_a_reason() {
        let dirs = Dirs::new("notpack");
        let fetcher = FakeFetch { bodies: HashMap::new() };
        std::fs::write(dirs.pack(), b"this is not a zip").unwrap();
        assert!(install(&dirs, &fetcher, &[]).unwrap_err().contains("не похож"));
        // a zip without the index
        let mut writer = zip::ZipWriter::new(File::create(dirs.pack()).unwrap());
        writer.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
        writer.finish().unwrap();
        assert!(install(&dirs, &fetcher, &[]).unwrap_err().contains("modrinth.index.json"));
        assert!(install(&Dirs { root: dirs.root.join("nope") }, &fetcher, &[]).is_err());
    }

    // A real pack from Modrinth, into a temporary folder; run by hand:
    // LC_MRPACK_FILE=path/to/pack.mrpack cargo test --lib real_pack -- --ignored --nocapture
    #[test]
    #[ignore = "needs network and a pack file"]
    fn real_pack_is_previewed_and_installed_into_a_temporary_folder() {
        let Ok(pack) = std::env::var("LC_MRPACK_FILE") else { panic!("set LC_MRPACK_FILE") };
        let preview = preview_of(Path::new(&pack), crate::config::MC_VERSION, crate::config::FABRIC_LOADER, false).unwrap();
        println!("{} {} | mods {} rp {} shaders {} overrides {} rejected {:?} server-only {} | {} KB | incompatible: {:?}", preview.name, preview.version, preview.mods, preview.resourcepacks, preview.shaderpacks, preview.overrides, preview.rejected, preview.client_unsupported, preview.total_size / 1024, preview.incompatible);
        let dirs = Dirs::new("real");
        let started = std::time::Instant::now();
        let outcome = install_into(
            Path::new(&pack),
            &dirs.game(),
            crate::config::MC_VERSION,
            crate::config::FABRIC_LOADER,
            &HttpFetch::new().unwrap(),
            &HashSet::new(),
            &|done, total, name| println!("  {done}/{total} {name}"),
        )
        .unwrap();
        println!("installed {} files ({} mods) in {:.1}s, kept {}, duplicates {:?}, rejected {}", outcome.files.len(), outcome.mods, started.elapsed().as_secs_f64(), outcome.kept_existing, outcome.already_installed, outcome.rejected);
        let on_disk = std::fs::read_dir(dirs.game().join("mods")).map(|d| d.flatten().count()).unwrap_or(0);
        println!("mods folder holds {on_disk} jars; staging folder gone: {}", !dirs.game().join(STAGING_DIR).exists());
        assert_eq!(on_disk, outcome.mods);
    }

    #[test]
    fn the_preview_counts_what_would_be_installed() {
        let dirs = Dirs::new("preview");
        let _ = write_pack(
            &dirs,
            DEPS,
            &[Entry { path: "mods/a.jar", body: jar_with_id("a") }, Entry { path: "shaderpacks/s.zip", body: b"s".to_vec() }],
            &[("overrides/config/a.toml", b"x"), ("overrides/mods/a.jar", b"same path as a download"), ("overrides/servers.dat", b"nope")],
        );
        let preview = preview_of(&dirs.pack(), "26.2", "0.19.5", true).unwrap();
        assert_eq!((preview.mods, preview.resourcepacks, preview.shaderpacks, preview.overrides), (1, 0, 1, 1));
        assert_eq!(preview.name, "Pack");
        assert!(preview.incompatible.is_none() && preview.replaces_import);
        let other = preview_of(&dirs.pack(), "1.20.1", "0.19.5", false).unwrap();
        assert!(other.incompatible.is_some());
    }
}
