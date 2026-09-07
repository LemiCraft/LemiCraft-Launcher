use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::config::API_BASE;
use crate::game::{game_dir, minecraft_dir};

#[derive(Deserialize, Serialize, Clone)]
pub struct ModCatalogEntry {
    id: String,
    #[serde(rename = "modrinthId")]
    modrinth_id: String,
    category: String,
    name: String,
    description: String,
    #[serde(default)]
    deps: Vec<String>,
    #[serde(default)]
    conflicts: Vec<String>,
    version: String,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    configs: Vec<String>,
}

fn timed_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

const CATALOG_FALLBACK_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);

fn catalog_cache_path() -> PathBuf {
    game_dir().join("mods_catalog_cache.json")
}

fn fetch_catalog_blocking(client: &reqwest::blocking::Client) -> Result<Vec<ModCatalogEntry>, String> {
    let live = (|| {
        let res = client.get(format!("{API_BASE}/mods")).send().map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Не удалось получить каталог модов ({})", res.status()));
        }
        res.json::<Vec<ModCatalogEntry>>().map_err(|err| err.to_string())
    })();
    match live {
        Ok(items) => {
            if let Ok(json) = serde_json::to_string(&items) {
                let _ = std::fs::write(catalog_cache_path(), json);
            }
            Ok(items)
        }
        Err(err) => crate::skin::read_cache_if_fresh(&catalog_cache_path(), CATALOG_FALLBACK_MAX_AGE)
            .and_then(|json| serde_json::from_str(&json).ok())
            .ok_or(err),
    }
}

#[tauri::command]
pub async fn get_cached_mod_catalog() -> Vec<ModCatalogEntry> {
    tauri::async_runtime::spawn_blocking(|| {
        std::fs::read_to_string(catalog_cache_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn get_mod_catalog() -> Result<Vec<ModCatalogEntry>, String> {
    tauri::async_runtime::spawn_blocking(|| fetch_catalog_blocking(&timed_client())).await.map_err(|err| err.to_string())?
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct InstalledMod {
    files: Vec<String>,
    version: String,
}

fn mods_state_path() -> PathBuf {
    game_dir().join("mods_state.json")
}

fn normalize_slug(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

// Picks up jars on disk with no mods_state.json entry (e.g. from the old C# launcher's shared
// folder) — matched by filename prefix against the catalog id/name, longest match wins
fn discover_unmanaged_mods(mods_dir: &Path, tracked_files: &HashSet<String>) -> HashMap<String, InstalledMod> {
    let catalog: Vec<ModCatalogEntry> =
        std::fs::read_to_string(catalog_cache_path()).ok().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
    let Ok(read_dir) = std::fs::read_dir(mods_dir) else { return HashMap::new() };

    let mut discovered = HashMap::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jar") {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if tracked_files.contains(filename) {
            continue;
        }
        let normalized_file = normalize_slug(filename);
        let best = catalog
            .iter()
            .filter_map(|m| {
                [normalize_slug(&m.id), normalize_slug(&m.name)]
                    .into_iter()
                    .filter(|c| !c.is_empty() && normalized_file.starts_with(c.as_str()))
                    .map(|c| c.len())
                    .max()
                    .map(|len| (len, m))
            })
            .max_by_key(|(len, _)| *len);
        if let Some((_, m)) = best {
            discovered.insert(m.id.clone(), InstalledMod { files: vec![filename.to_string()], version: m.version.clone() });
        }
    }
    discovered
}

fn load_state() -> HashMap<String, InstalledMod> {
    let raw: HashMap<String, InstalledMod> = std::fs::read_to_string(mods_state_path())
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    // Drop entries whose files no longer exist — reflects the folder, not stale bookkeeping
    let mods_dir = minecraft_dir().join("mods");
    let mut state: HashMap<String, InstalledMod> = raw
        .into_iter()
        .filter_map(|(id, entry)| {
            let existing: Vec<String> = entry.files.into_iter().filter(|f| mods_dir.join(f).is_file()).collect();
            if existing.is_empty() {
                None
            } else {
                Some((id, InstalledMod { files: existing, version: entry.version }))
            }
        })
        .collect();

    let tracked_files: HashSet<String> = state
        .values()
        .flat_map(|e| e.files.iter().cloned())
        .chain(load_official_pack_state().map(|p| p.files).unwrap_or_default())
        .collect();
    for (id, discovered_entry) in discover_unmanaged_mods(&mods_dir, &tracked_files) {
        state.entry(id).or_insert(discovered_entry);
    }
    state
}

fn save_state(state: &HashMap<String, InstalledMod>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    std::fs::write(mods_state_path(), json).map_err(|err| err.to_string())
}

#[derive(Serialize, Default)]
pub struct InstalledModsInfo {
    // Individually tracked — real per-id file ownership, safe to uninstall one at a time
    individual: HashMap<String, String>,
    // Only present via the official pack's shared file list — see uninstall_mod
    pack_ids: Vec<String>,
}

#[tauri::command]
pub async fn get_installed_mods() -> InstalledModsInfo {
    tauri::async_runtime::spawn_blocking(|| {
        let individual: HashMap<String, String> = load_state().into_iter().map(|(id, entry)| (id, entry.version)).collect();
        let pack_ids = load_official_pack_state().map(|p| p.minimal_mods).unwrap_or_default();
        InstalledModsInfo { individual, pack_ids }
    })
    .await
    .unwrap_or_default()
}

fn resolve_deps(ids: &[String], catalog: &[ModCatalogEntry], out: &mut HashSet<String>) {
    let by_id: HashMap<&str, &ModCatalogEntry> = catalog.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut stack: Vec<String> = ids.to_vec();
    while let Some(id) = stack.pop() {
        if !out.insert(id.clone()) {
            continue;
        }
        if let Some(entry) = by_id.get(id.as_str()) {
            for dep in &entry.deps {
                stack.push(dep.clone());
            }
        }
    }
}

// Bounds a malicious/compromised zip that's small on the wire but decompresses huge
const MAX_DECOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 20_000;

// Zip-slip safe: only entries under mods/ whose enclosed_name resolves cleanly are extracted
fn extract_mods_zip(bytes: &[u8], mods_dir: &Path) -> Result<Vec<String>, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|err| err.to_string())?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err("Архив содержит слишком много файлов".to_string());
    }
    std::fs::create_dir_all(mods_dir).map_err(|err| err.to_string())?;

    let mut extracted = Vec::new();
    let mut total_written: u64 = 0;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|err| err.to_string())?;
        let Some(enclosed) = file.enclosed_name() else { continue };
        let Ok(rel) = enclosed.strip_prefix("mods") else { continue };
        if rel.as_os_str().is_empty() || file.is_dir() {
            continue;
        }
        let dest = mods_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|err| err.to_string())?;
        // Caps actual bytes copied regardless of what the entry's declared size claims
        let remaining = MAX_DECOMPRESSED_BYTES.saturating_sub(total_written);
        let mut limited = std::io::Read::take(&mut file, remaining + 1);
        let copied = std::io::copy(&mut limited, &mut out).map_err(|err| err.to_string())?;
        total_written += copied;
        if total_written > MAX_DECOMPRESSED_BYTES {
            return Err("Архив превышает допустимый размер после распаковки — установка отменена".to_string());
        }
        extracted.push(rel.to_string_lossy().to_string());
    }
    Ok(extracted)
}

#[derive(Serialize)]
struct ExportRequest<'a> {
    mods: &'a [String],
    configs: bool,
    resourcepacks: Vec<String>,
    shaders: Vec<String>,
    #[serde(rename = "includeOptions")]
    include_options: bool,
}

#[derive(Deserialize)]
struct ExportResponse {
    code: String,
}

fn export_selection(client: &reqwest::blocking::Client, ids: &[String]) -> Result<String, String> {
    let body = ExportRequest { mods: ids, configs: false, resourcepacks: vec![], shaders: vec![], include_options: false };
    let res = client.post(format!("{API_BASE}/mods/export")).json(&body).send().map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Не удалось создать код сборки ({})", res.status()));
    }
    res.json::<ExportResponse>().map(|r| r.code).map_err(|err| err.to_string())
}

// Generous cap for a single mod/pack download — guards against a compromised endpoint
// claiming an absurd body size rather than any realistic legitimate mod file
const MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024;

fn download_bytes(client: &reqwest::blocking::Client, code: &str) -> Result<Vec<u8>, String> {
    let res = client.get(format!("{API_BASE}/mods/download/{code}")).send().map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Не удалось скачать мод ({})", res.status()));
    }
    if res.content_length().is_some_and(|len| len > MAX_DOWNLOAD_BYTES) {
        return Err("Файл мода подозрительно велик".to_string());
    }
    let bytes = res.bytes().map_err(|err| err.to_string())?;
    // Content-Length is absent for a chunked response, so the check above alone doesn't bound
    // that case — this backstops it the same way fetch_skin_data_uri's post-download check does
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err("Файл мода подозрительно велик".to_string());
    }
    Ok(bytes.to_vec())
}

#[tauri::command]
pub async fn export_mod_selection(ids: Vec<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || export_selection(&timed_client(), &ids)).await.map_err(|err| err.to_string())?
}

#[derive(Deserialize)]
struct ImportResponse {
    mods: Vec<String>,
    #[serde(default)]
    configs: bool,
}

#[derive(Serialize, Clone)]
pub struct ImportPreviewItem {
    id: String,
    name: String,
    version: String,
    resolved: bool,
}

#[derive(Serialize)]
pub struct ImportPreview {
    items: Vec<ImportPreviewItem>,
    configs: bool,
}

#[tauri::command]
pub async fn preview_import_code(code: String) -> Result<ImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = timed_client();
        let res = client.get(format!("{API_BASE}/mods/import/{code}")).send().map_err(|err| err.to_string())?;
        if res.status().as_u16() == 404 {
            return Err("Код не найден или истёк".to_string());
        }
        if !res.status().is_success() {
            return Err(format!("Не удалось загрузить сборку ({})", res.status()));
        }
        let parsed: ImportResponse = res.json().map_err(|err| err.to_string())?;

        let catalog = fetch_catalog_blocking(&client).unwrap_or_default();
        let by_id: HashMap<&str, &ModCatalogEntry> = catalog.iter().map(|m| (m.id.as_str(), m)).collect();
        let items = parsed
            .mods
            .into_iter()
            .map(|id| match by_id.get(id.as_str()) {
                Some(entry) => ImportPreviewItem { id: id.clone(), name: entry.name.clone(), version: entry.version.clone(), resolved: true },
                None => ImportPreviewItem { name: id.clone(), version: "неизвестно".to_string(), id, resolved: false },
            })
            .collect();
        Ok(ImportPreview { items, configs: parsed.configs })
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn install_mods(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = timed_client();
        let catalog = fetch_catalog_blocking(&client)?;
        let by_id: HashMap<&str, &ModCatalogEntry> = catalog.iter().map(|m| (m.id.as_str(), m)).collect();

        let mut resolved = HashSet::new();
        resolve_deps(&ids, &catalog, &mut resolved);

        let mut state = load_state();

        // Checked against the full resulting set (existing + newly resolved deps), not just the
        // clicked id — the frontend only inspects the clicked entry's own `conflicts` list
        let final_set: HashSet<&str> = state.keys().map(String::as_str).chain(resolved.iter().map(String::as_str)).collect();
        for id in &final_set {
            if let Some(entry) = by_id.get(id) {
                if let Some(conflict) = entry.conflicts.iter().find(|c| final_set.contains(c.as_str())) {
                    let conflict_name = by_id.get(conflict.as_str()).map(|m| m.name.as_str()).unwrap_or(conflict.as_str());
                    return Err(format!("«{}» конфликтует с «{}» — нельзя установить их вместе", entry.name, conflict_name));
                }
            }
        }

        let mods_dir = minecraft_dir().join("mods");
        let to_install: Vec<String> = resolved.into_iter().filter(|id| !state.contains_key(id)).collect();
        let total = to_install.len();

        for (i, id) in to_install.iter().enumerate() {
            let _ = app.emit("mods-progress", serde_json::json!({ "current": i + 1, "total": total, "id": id }));
            let code = export_selection(&client, std::slice::from_ref(id))?;
            let bytes = download_bytes(&client, &code)?;
            let files = extract_mods_zip(&bytes, &mods_dir)?;
            if !files.is_empty() {
                let version = by_id.get(id.as_str()).map(|m| m.version.clone()).unwrap_or_default();
                state.insert(id.clone(), InstalledMod { files, version });
                // Persisted after every mod, not just once at the end — a later mod failing
                // in this same batch must not discard the ones that already succeeded
                save_state(&state)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn uninstall_mod(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = load_state();

        // Not individually tracked — either untouched, or only present as part of the
        // official pack, which has no per-id file mapping and can't be split apart safely
        if !state.contains_key(&id) {
            if let Some(pack) = load_official_pack_state() {
                if pack.minimal_mods.iter().any(|m| m == &id) {
                    return Err("Этот мод входит в сборку LemiSborka — уберите всю сборку целиком в её карточке выше".to_string());
                }
            }
            return Ok(());
        }

        let client = timed_client();
        let catalog = fetch_catalog_blocking(&client).unwrap_or_default();
        let dependents: Vec<String> = catalog.iter().filter(|m| state.contains_key(&m.id) && m.deps.contains(&id)).map(|m| m.name.clone()).collect();
        if !dependents.is_empty() {
            return Err(format!("От этого мода зависят: {}", dependents.join(", ")));
        }

        // Delete the files BEFORE touching `state` — on partial failure the mod must stay
        // tracked so retry works, rather than being marked gone while its jar still exists
        let entry = state.get(&id).cloned().unwrap_or_default();
        let mods_dir = minecraft_dir().join("mods");
        let mut delete_failed = false;
        for f in &entry.files {
            if let Err(err) = std::fs::remove_file(mods_dir.join(f)) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    delete_failed = true;
                }
            }
        }
        if delete_failed {
            return Err("Не удалось удалить один или несколько файлов мода — возможно, игра ещё запущена или файл занят другой программой".to_string());
        }
        state.remove(&id);
        save_state(&state)?;
        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

// --- Official curated pack ("LemiSborka") — separate static-zip endpoint, own version tracking ---

#[derive(Deserialize)]
struct OfficialPackResponse {
    success: bool,
    version: String,
    name: String,
    #[serde(rename = "downloadUrl")]
    download_url: String,
    #[serde(rename = "fileSize")]
    file_size: u64,
    #[serde(default)]
    changelog: Vec<String>,
    #[serde(rename = "minimalMods", default)]
    minimal_mods: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct OfficialPackInfo {
    version: String,
    name: String,
    download_url: String,
    file_size: u64,
    changelog: Vec<String>,
    minimal_mods: Vec<String>,
}

#[tauri::command]
pub async fn get_official_pack() -> Result<Option<OfficialPackInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let client = timed_client();
        let res = client.get(format!("{API_BASE}/launcher/modpack/version")).send().map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Ok(None);
        }
        let parsed: OfficialPackResponse = res.json().map_err(|err| err.to_string())?;
        if !parsed.success {
            return Ok(None);
        }
        Ok(Some(OfficialPackInfo {
            version: parsed.version,
            name: parsed.name,
            download_url: parsed.download_url,
            file_size: parsed.file_size,
            changelog: parsed.changelog,
            minimal_mods: parsed.minimal_mods,
        }))
    })
    .await
    .map_err(|err| err.to_string())?
}

// No per-file->id mapping in the pack zip, so it can't be split into per-id mods_state.json
// entries like install_mods — tracked as one unit; ids only look installed via get_installed_mods
#[derive(Serialize, Deserialize, Clone, Default)]
struct OfficialPackState {
    files: Vec<String>,
    minimal_mods: Vec<String>,
    version: String,
}

fn official_pack_state_path() -> PathBuf {
    game_dir().join("official_pack_state.json")
}

fn load_official_pack_state() -> Option<OfficialPackState> {
    let state: OfficialPackState =
        std::fs::read_to_string(official_pack_state_path()).ok().and_then(|json| serde_json::from_str(&json).ok())?;
    // Mirrors load_state()'s self-healing — if no tracked file is present, stop reporting the
    // pack as installed (`.any()` on an empty list is false, so an empty list hits this too)
    let mods_dir = minecraft_dir().join("mods");
    if !state.files.iter().any(|f| mods_dir.join(f).is_file()) {
        return None;
    }
    Some(state)
}

fn save_official_pack_state(state: &OfficialPackState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    std::fs::write(official_pack_state_path(), json).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn get_installed_official_pack_version() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| load_official_pack_state().map(|p| p.version)).await.unwrap_or(None)
}

#[tauri::command]
pub async fn uninstall_official_pack() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        // Delete files BEFORE removing the state file — on a partial failure the pack must
        // stay tracked so a retry (or load_official_pack_state's self-heal) can still see it
        if let Some(pack) = load_official_pack_state() {
            let mods_dir = minecraft_dir().join("mods");
            let mut delete_failed = false;
            for f in &pack.files {
                if let Err(err) = std::fs::remove_file(mods_dir.join(f)) {
                    if err.kind() != std::io::ErrorKind::NotFound {
                        delete_failed = true;
                    }
                }
            }
            if delete_failed {
                return Err(
                    "Не удалось удалить один или несколько файлов сборки — возможно, игра ещё запущена или файл занят другой программой"
                        .to_string(),
                );
            }
        }
        let _ = std::fs::remove_file(official_pack_state_path());
        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn apply_official_pack(app: AppHandle, download_url: String, version: String, minimal_mods: Vec<String>, file_size: u64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // The pack bundles many mods and can take longer than timed_client()'s 20s timeout, so this
        // retries a few bounded attempts instead of one long wait — same shape as game.rs's install()
        const PACK_DOWNLOAD_ATTEMPTS: u32 = 3;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|err| err.to_string())?;
        let _ = app.emit("mods-progress", serde_json::json!({ "stage": "downloading-pack" }));

        let mut last_error = String::new();
        let mut downloaded = None;
        for attempt in 1..=PACK_DOWNLOAD_ATTEMPTS {
            if attempt > 1 {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            let attempt_result = (|| -> Result<Vec<u8>, String> {
                let res = client.get(&download_url).send().map_err(|err| err.to_string())?;
                if !res.status().is_success() {
                    return Err(format!("Не удалось скачать сборку ({})", res.status()));
                }
                Ok(res.bytes().map_err(|err| err.to_string())?.to_vec())
            })();
            match attempt_result {
                Ok(b) => {
                    downloaded = Some(b);
                    break;
                }
                Err(err) => last_error = err,
            }
        }
        let bytes = downloaded.ok_or_else(|| format!("Не удалось скачать сборку после {PACK_DOWNLOAD_ATTEMPTS} попыток: {last_error}"))?;
        if file_size > 0 && bytes.len() as u64 != file_size {
            return Err("Скачанный файл сборки повреждён или неполон (размер не совпадает)".to_string());
        }

        let _ = app.emit("mods-progress", serde_json::json!({ "stage": "extracting-pack" }));
        let mods_dir = minecraft_dir().join("mods");
        let previous = load_official_pack_state();
        let files = extract_mods_zip(&bytes, &mods_dir)?;

        // Files that existed in the previous pack version but aren't in this one must be
        // removed explicitly — extraction only ever adds/overwrites, it never deletes
        if let Some(prev) = &previous {
            for f in &prev.files {
                if !files.contains(f) {
                    let _ = std::fs::remove_file(mods_dir.join(f));
                }
            }
        }

        save_official_pack_state(&OfficialPackState { files, minimal_mods, version })?;

        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?
}

// An import code's zip also carries resourcepacks/shaderpacks/config/options.txt alongside mods/
// (installed separately via install_mods) — the rest has no per-mod owner, tracked as one unit here
#[derive(Serialize, Deserialize, Clone, Default)]
struct ImportExtrasState {
    files: Vec<String>,
}

fn import_extras_state_path() -> PathBuf {
    game_dir().join("import_extras_state.json")
}

fn load_import_extras_state() -> ImportExtrasState {
    std::fs::read_to_string(import_extras_state_path()).ok().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default()
}

fn save_import_extras_state(state: &ImportExtrasState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    std::fs::write(import_extras_state_path(), json).map_err(|err| err.to_string())
}

const IMPORT_EXTRA_DIRS: [&str; 3] = ["resourcepacks", "shaderpacks", "config"];

// Same zip-slip protections as extract_mods_zip, but keyed on the opposite set of entries —
// mods/ is skipped here since install_mods already downloads and tracks those separately
fn extract_import_extras(bytes: &[u8], minecraft_dir: &Path) -> Result<Vec<String>, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|err| err.to_string())?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err("Архив содержит слишком много файлов".to_string());
    }

    let mut extracted = Vec::new();
    let mut total_written: u64 = 0;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|err| err.to_string())?;
        let Some(enclosed) = file.enclosed_name() else { continue };
        if file.is_dir() {
            continue;
        }
        let is_extra = enclosed == Path::new("options.txt")
            || IMPORT_EXTRA_DIRS.iter().any(|dir| enclosed.starts_with(dir));
        if !is_extra {
            continue;
        }
        let dest = minecraft_dir.join(&enclosed);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|err| err.to_string())?;
        let remaining = MAX_DECOMPRESSED_BYTES.saturating_sub(total_written);
        let mut limited = std::io::Read::take(&mut file, remaining + 1);
        let copied = std::io::copy(&mut limited, &mut out).map_err(|err| err.to_string())?;
        total_written += copied;
        if total_written > MAX_DECOMPRESSED_BYTES {
            return Err("Архив превышает допустимый размер после распаковки — установка отменена".to_string());
        }
        extracted.push(enclosed.to_string_lossy().to_string());
    }
    Ok(extracted)
}

#[tauri::command]
pub async fn apply_import_extras(code: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = timed_client();
        let bytes = download_bytes(&client, &code)?;
        let minecraft_dir = minecraft_dir();
        let new_files = extract_import_extras(&bytes, &minecraft_dir)?;

        let previous = load_import_extras_state();
        for f in &previous.files {
            if !new_files.contains(f) {
                let _ = std::fs::remove_file(minecraft_dir.join(f));
            }
        }

        save_import_extras_state(&ImportExtrasState { files: new_files })
    })
    .await
    .map_err(|err| err.to_string())?
}
