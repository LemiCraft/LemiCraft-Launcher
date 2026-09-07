use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::config::API_BASE;
use crate::game::game_dir;

pub(crate) fn cache_key(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn read_cache_if_fresh(path: &std::path::Path, ttl: Duration) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    if SystemTime::now().duration_since(modified).ok()? > ttl {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn timed_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

// skinview3d needs a same-origin texture; the skin host sends no CORS headers, so this
// fetches server-side and returns a data: URI instead
const IMAGE_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const MAX_SKIN_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

fn image_cache_dir() -> PathBuf {
    game_dir().join("skin_image_cache")
}

fn is_private_or_local(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast(),
        std::net::IpAddr::V6(v6) => {
            // An IPv4-mapped address (::ffff:a.b.c.d) must be judged by the IPv4 rules above it
            // wraps, or a loopback/private v4 target sails through this branch unblocked
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_or_local(std::net::IpAddr::V4(v4));
            }
            v6.is_loopback() || v6.is_unspecified() || v6.is_unicast_link_local() || v6.is_unique_local()
        }
    }
}

// url is backend-controlled — resolves+validates the host once and returns the address to pin,
// so a second DNS lookup at request time can't flip the target after validation (DNS rebinding)
fn resolve_and_validate_skin_host(url: &str) -> Result<(std::net::SocketAddr, String), String> {
    let bad = || "Недопустимый адрес изображения скина".to_string();
    let parsed = reqwest::Url::parse(url).map_err(|_| bad())?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(bad());
    }
    let host = parsed.host_str().ok_or_else(bad)?.to_string();
    let port = parsed.port_or_known_default().unwrap_or(443);
    use std::net::ToSocketAddrs;
    let addrs: Vec<_> = (host.as_str(), port).to_socket_addrs().map_err(|_| bad())?.collect();
    if addrs.is_empty() || addrs.iter().any(|addr| is_private_or_local(addr.ip())) {
        return Err(bad());
    }
    let addr = addrs[0];
    Ok((addr, host))
}

#[tauri::command]
pub async fn fetch_skin_data_uri(url: String, force: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (addr, host) = resolve_and_validate_skin_host(&url)?;

        let cache_path = image_cache_dir().join(cache_key(&url));
        if !force {
            if let Some(cached) = read_cache_if_fresh(&cache_path, IMAGE_CACHE_TTL) {
                return Ok(cached);
            }
        }

        // Pinned to the exact address validated above — without this, reqwest would re-resolve
        // the host itself at connect time, reopening the DNS-rebinding gap the pin closes
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .resolve(&host, addr)
            .build()
            .map_err(|err| err.to_string())?;
        let res = client.get(&url).send().map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Не удалось загрузить скин ({})", res.status()));
        }
        if res.content_length().is_some_and(|len| len > MAX_SKIN_IMAGE_BYTES) {
            return Err("Файл скина подозрительно велик".to_string());
        }
        let content_type = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .to_string();
        if !content_type.starts_with("image/") {
            return Err("Ответ не является изображением".to_string());
        }
        let bytes = res.bytes().map_err(|err| err.to_string())?;
        if bytes.len() as u64 > MAX_SKIN_IMAGE_BYTES {
            return Err("Файл скина подозрительно велик".to_string());
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_uri = format!("data:{content_type};base64,{encoded}");

        if std::fs::create_dir_all(image_cache_dir()).is_ok() {
            let _ = std::fs::write(&cache_path, &data_uri);
        }

        Ok(data_uri)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[derive(Deserialize, Serialize, Clone)]
pub struct LicenseSkinItem {
    id: i64,
    #[serde(rename = "elybyId")]
    pub(crate) elyby_id: Option<i64>,
    name: String,
    model: String,
    #[serde(rename = "fileUrl")]
    file_url: String,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: String,
    #[serde(rename = "isActive")]
    is_active: bool,
    #[serde(rename = "addedAt", default)]
    added_at: Option<String>,
}

#[derive(Deserialize)]
struct UserSkinsResponse {
    #[serde(default)]
    skins: Vec<LicenseSkinItem>,
}

const SKINS_LIST_FALLBACK_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

fn skins_list_cache_path(username: &str) -> PathBuf {
    game_dir().join("skins_list_cache").join(format!("license_{}.json", cache_key(username)))
}

pub(crate) fn get_license_skins(username: &str) -> Result<Vec<LicenseSkinItem>, String> {
    let cache_path = skins_list_cache_path(username);
    let client = timed_client();
    let live = (|| {
        let res = client
            .get(format!("{API_BASE}/launcher/skins/user/{username}"))
            .send()
            .map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Не удалось получить список скинов ({})", res.status()));
        }
        let parsed: UserSkinsResponse = res.json().map_err(|err| err.to_string())?;
        Ok(parsed.skins)
    })();

    match live {
        Ok(items) => {
            if let Ok(json) = serde_json::to_string(&items) {
                if std::fs::create_dir_all(cache_path.parent().unwrap()).is_ok() {
                    let _ = std::fs::write(&cache_path, json);
                }
            }
            Ok(items)
        }
        Err(err) => read_cache_if_fresh(&cache_path, SKINS_LIST_FALLBACK_MAX_AGE)
            .and_then(|json| serde_json::from_str(&json).ok())
            .ok_or(err),
    }
}

#[derive(Deserialize)]
struct SuccessResponse {
    #[serde(default)]
    success: bool,
}

fn upload_license_skin(file_bytes: Vec<u8>, file_name: String, name: String, model: String, username: &str) -> Result<bool, String> {
    let client = timed_client();
    let part = reqwest::blocking::multipart::Part::bytes(file_bytes).file_name(file_name);
    let form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("name", name)
        .text("model", model)
        .text("username", username.to_string());

    let res = client
        .post(format!("{API_BASE}/launcher/skins/upload"))
        .multipart(form)
        .send()
        .map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Не удалось загрузить скин ({})", res.status()));
    }
    let parsed: SuccessResponse = res.json().map_err(|err| err.to_string())?;
    Ok(parsed.success)
}

fn apply_license_skin(
    skin_id: i64,
    username: &str,
    uuid: Option<&str>,
    provider: Option<&str>,
    access_token: Option<&str>,
) -> Result<bool, String> {
    let client = timed_client();
    let body = serde_json::json!({
        "skinId": skin_id,
        "username": username,
        "uuid": uuid,
        "provider": provider,
        "accessToken": access_token,
    });
    let res = client
        .post(format!("{API_BASE}/launcher/skins/apply"))
        .json(&body)
        .send()
        .map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Не удалось применить скин ({})", res.status()));
    }
    let parsed: SuccessResponse = res.json().map_err(|err| err.to_string())?;
    Ok(parsed.success)
}

fn delete_license_skin(skin_id: i64, username: &str) -> Result<bool, String> {
    let client = timed_client();
    let body = serde_json::json!({ "skinId": skin_id, "username": username });
    let res = client
        .post(format!("{API_BASE}/launcher/skins/delete"))
        .json(&body)
        .send()
        .map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Не удалось удалить скин ({})", res.status()));
    }
    let parsed: SuccessResponse = res.json().map_err(|err| err.to_string())?;
    Ok(parsed.success)
}

#[tauri::command]
pub async fn get_current_account_skins() -> Result<Vec<LicenseSkinItem>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (_, username, _, _) = crate::auth::load_current_session_full().ok_or("Не удалось определить текущий аккаунт")?;
        get_license_skins(&username)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn get_cached_current_account_skins() -> Vec<LicenseSkinItem> {
    tauri::async_runtime::spawn_blocking(|| {
        let Some((_, username, _, _)) = crate::auth::load_current_session_full() else {
            return Vec::new();
        };
        let cache_path = skins_list_cache_path(&username);
        std::fs::read_to_string(cache_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn upload_current_account_skin(file_bytes: Vec<u8>, file_name: String, name: String, model: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, username, _, _) = crate::auth::load_current_session_full().ok_or("Не удалось определить текущий аккаунт")?;
        upload_license_skin(file_bytes, file_name, name, model, &username)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn apply_current_account_skin(skin_id: i64) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (uuid, username, access_token, provider) =
            crate::auth::load_current_session_full().ok_or("Не удалось определить текущий аккаунт")?;
        apply_license_skin(skin_id, &username, Some(&uuid), Some(&provider), Some(&access_token))
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn delete_current_account_skin(skin_id: i64) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, username, _, _) = crate::auth::load_current_session_full().ok_or("Не удалось определить текущий аккаунт")?;
        delete_license_skin(skin_id, &username)
    })
    .await
    .map_err(|err| err.to_string())?
}
