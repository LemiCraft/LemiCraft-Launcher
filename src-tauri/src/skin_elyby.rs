// Ely.by has no skin-management API — this drives its website's own
// (undocumented) endpoints with a real logged-in session's cookies

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

use crate::game::game_dir;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

fn timed_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

fn skin_session_path() -> PathBuf {
    game_dir().join("elyby_skin_session.json")
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct SkinSession {
    php_sess_id: Option<String>,
    identity: Option<String>,
}

impl SkinSession {
    fn is_valid(&self) -> bool {
        self.php_sess_id.is_some() || self.identity.is_some()
    }

    fn cookie_header(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = &self.php_sess_id {
            parts.push(format!("PHPSESSID={v}"));
        }
        if let Some(v) = &self.identity {
            parts.push(format!("identity={v}"));
        }
        parts.join("; ")
    }
}

// Encrypted the same way auth.rs protects account.sec — this session is just as capable of
// acting as the user on ely.by, so it deserves the same at-rest protection, not plaintext
fn load_skin_session() -> SkinSession {
    std::fs::read(skin_session_path())
        .ok()
        .and_then(|bytes| crate::auth::decrypt_bytes(&bytes))
        .and_then(|plain| String::from_utf8(plain).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_skin_session(session: &SkinSession) -> Result<(), String> {
    std::fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(session).unwrap();
    std::fs::write(skin_session_path(), crate::auth::encrypt_bytes(json.as_bytes())).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn has_elyby_skin_session() -> bool {
    load_skin_session().is_valid()
}

#[tauri::command]
pub fn clear_elyby_skin_session() -> Result<(), String> {
    std::fs::remove_file(skin_session_path()).ok();
    Ok(())
}

#[tauri::command]
pub async fn open_elyby_web_login(app: AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("elyby-web") {
        let _ = existing.set_focus();
        return Ok(());
    }

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let app_handle = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| {
            let url = Url::parse("https://ely.by/authorization/login").map_err(|err| err.to_string())?;
            let builder = WebviewWindowBuilder::new(&app_handle, "elyby-web", WebviewUrl::External(url))
                .title("Вход на ely.by")
                .inner_size(900.0, 650.0)
                .center();
            let builder = match app_handle.get_webview_window("main") {
                Some(main) => builder.owner(&main).map_err(|err| err.to_string())?,
                None => builder,
            };
            builder.build().map_err(|err| err.to_string())?;
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(|err| err.to_string())?;

    rx.await.map_err(|_| "Канал закрылся до ответа от главного потока".to_string())??;
    let _ = app.emit("elyby-login-opened", ());

    let app_poll = app.clone();
    tauri::async_runtime::spawn(async move {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;

            if app_poll.get_webview_window("elyby-web").is_none() {
                let _ = app_poll.emit("elyby-login-error", "Окно входа закрыто");
                return;
            }

            if let Ok(session) = read_session_cookies(&app_poll).await {
                if session.is_valid() {
                    if save_skin_session(&session).is_ok() {
                        if let Some(window) = app_poll.get_webview_window("elyby-web") {
                            let _ = window.close();
                        }
                        let _ = app_poll.emit("elyby-login-success", ());
                        return;
                    }
                }
            }

            if std::time::Instant::now() > deadline {
                if let Some(window) = app_poll.get_webview_window("elyby-web") {
                    let _ = window.close();
                }
                let _ = app_poll.emit("elyby-login-error", "Истекло время ожидания входа");
                return;
            }
        }
    });

    Ok(())
}

async fn read_session_cookies(app: &AppHandle) -> Result<SkinSession, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let app_handle = app.clone();
    app.run_on_main_thread(move || {
        let result = app_handle
            .get_webview_window("elyby-web")
            .ok_or_else(|| "Окно входа уже закрыто".to_string())
            .and_then(|window| {
                window
                    .cookies_for_url(Url::parse("https://ely.by").unwrap())
                    .map_err(|err| err.to_string())
            });
        let _ = tx.send(result);
    })
    .map_err(|err| err.to_string())?;

    let cookies = rx.await.map_err(|_| "Канал закрылся до ответа от главного потока".to_string())??;

    let mut session = SkinSession::default();
    for cookie in cookies {
        match cookie.name() {
            "PHPSESSID" => session.php_sess_id = Some(cookie.value().to_string()),
            "identity" => session.identity = Some(cookie.value().to_string()),
            _ => {}
        }
    }
    Ok(session)
}

#[tauri::command]
pub async fn finish_elyby_web_login(app: AppHandle) -> Result<(), String> {
    let session = read_session_cookies(&app).await?;
    if !session.is_valid() {
        return Err("Не удалось получить сессию — убедитесь, что вы вошли на сайте ely.by, и попробуйте снова".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || save_skin_session(&session))
        .await
        .map_err(|err| err.to_string())??;
    if let Some(window) = app.get_webview_window("elyby-web") {
        let _ = window.close();
    }
    Ok(())
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ElySkinItem {
    id: i64,
    #[serde(rename = "skin_url")]
    skin_url: String,
    #[serde(rename = "is_slim", default)]
    is_slim: bool,
    #[serde(rename = "isActive", default)]
    is_active: bool,
}

#[derive(Deserialize)]
struct ElySkinsList {
    #[serde(default)]
    items: Vec<ElySkinItem>,
}

fn session_or_err() -> Result<SkinSession, String> {
    let session = load_skin_session();
    if !session.is_valid() {
        return Err("Нет сессии ely.by — сначала войдите через «Управлять на ely.by»".to_string());
    }
    Ok(session)
}

// ely.by's action endpoints return {"error":"error_login"} on an expired session
fn check_session_expired(body: &str) -> Result<(), String> {
    if body.contains("\"error\":\"error_login\"") {
        std::fs::remove_file(skin_session_path()).ok();
        return Err("Сессия ely.by истекла — войдите заново через «Управлять на ely.by»".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_elyby_skins() -> Result<Vec<ElySkinItem>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (_, username, _, _) = crate::auth::load_current_session_full().ok_or("Не удалось определить текущий аккаунт")?;
        let session = session_or_err()?;
        let client = timed_client();
        fetch_elyby_skins(&client, &session, &username)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn get_cached_elyby_skins() -> Vec<ElySkinItem> {
    tauri::async_runtime::spawn_blocking(|| {
        let Some((_, username, _, _)) = crate::auth::load_current_session_full() else {
            return Vec::new();
        };
        std::fs::read_to_string(elyby_skins_cache_path(&username))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

const ELYBY_SKINS_FALLBACK_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);

fn elyby_skins_cache_path(username: &str) -> PathBuf {
    game_dir().join("skins_list_cache").join(format!("elyby_{}.json", crate::skin::cache_key(username)))
}

fn fetch_elyby_skins(client: &reqwest::blocking::Client, session: &SkinSession, username: &str) -> Result<Vec<ElySkinItem>, String> {
    let cache_path = elyby_skins_cache_path(username);
    let live = fetch_elyby_skins_live(client, session, username);

    let mut list = match live {
        Ok(items) => {
            if let Ok(json) = serde_json::to_string(&items) {
                if std::fs::create_dir_all(cache_path.parent().unwrap()).is_ok() {
                    let _ = std::fs::write(&cache_path, json);
                }
            }
            ElySkinsList { items }
        }
        Err(err) => {
            let fallback = crate::skin::read_cache_if_fresh(&cache_path, ELYBY_SKINS_FALLBACK_MAX_AGE)
                .and_then(|json| serde_json::from_str::<Vec<ElySkinItem>>(&json).ok());
            match fallback {
                Some(items) => ElySkinsList { items },
                None => return Err(err),
            }
        }
    };

    if let Some(active_hash) = fetch_active_skin_hash(client, username) {
        for item in &mut list.items {
            item.is_active = skin_url_hash(&item.skin_url) == Some(active_hash.as_str());
        }
    }

    // Backfills LemiCraft's DB for skins uploaded outside this launcher (no name/thumbnail otherwise)
    if let Ok(known) = crate::skin::get_license_skins(username) {
        let known_elyby_ids: std::collections::HashSet<i64> = known.iter().filter_map(|s| s.elyby_id).collect();
        for item in &list.items {
            if known_elyby_ids.contains(&item.id) {
                continue;
            }
            sync_elyby_skin_to_db(client, username, item, &format!("Скин #{}", item.id));
        }
    }

    Ok(list.items)
}

fn fetch_elyby_skins_live(client: &reqwest::blocking::Client, session: &SkinSession, username: &str) -> Result<Vec<ElySkinItem>, String> {
    let url = format!("https://ely.by/skins?uploader={}", urlencoding_encode(username));
    let html = client
        .get(&url)
        .header("Cookie", session.cookie_header())
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|err| err.to_string())?
        .text()
        .map_err(|err| err.to_string())?;

    // The page embeds the list as `alight.service.skins = {...json...};` — any failure to find/parse
    // this must be a real Err, not Ok(empty), or the caller's cache-write would erase the real list
    let parse_err = || "Не удалось разобрать страницу ely.by — возможно, сайт временно недоступен или изменил вёрстку".to_string();
    let marker = html.find("alight.service.skins").ok_or_else(parse_err)?;
    let eq_offset = html[marker..].find('=').ok_or_else(parse_err)?;
    let json_start = marker + eq_offset + 1;
    let mut stream = serde_json::Deserializer::from_str(html[json_start..].trim_start()).into_iter::<ElySkinsList>();
    let list = stream.next().and_then(|r| r.ok()).ok_or_else(parse_err)?;
    Ok(list.items)
}

fn sync_elyby_skin_to_db(client: &reqwest::blocking::Client, username: &str, item: &ElySkinItem, name: &str) {
    let model = if item.is_slim { "alex" } else { "steve" };
    let sync_body = serde_json::json!({
        "elybyId": item.id,
        "username": username,
        "name": name,
        "skinUrl": item.skin_url,
        "model": model,
    });
    let _ = client.post(format!("{}/launcher/skins/sync", crate::config::API_BASE)).json(&sync_body).send();
}

#[derive(Deserialize)]
struct TexturesResponse {
    #[serde(rename = "SKIN")]
    skin: Option<TextureEntry>,
}

#[derive(Deserialize)]
struct TextureEntry {
    url: String,
}

fn skin_url_hash(url: &str) -> Option<&str> {
    url.rsplit('/').next()
}

// Nickname-only public endpoint, no UUID lookup exists for it
fn fetch_active_skin_hash(client: &reqwest::blocking::Client, username: &str) -> Option<String> {
    let url = format!("https://skinsystem.ely.by/textures/{}", urlencoding_encode(username));
    let res = client.get(&url).header("User-Agent", USER_AGENT).send().ok()?;
    if res.status() == reqwest::StatusCode::NO_CONTENT {
        return None;
    }
    let body: TexturesResponse = res.json().ok()?;
    skin_url_hash(&body.skin?.url).map(|s| s.to_string())
}

fn wear_elyby_skin_blocking(skin_id: i64) -> Result<bool, String> {
    let session = session_or_err()?;
    let client = timed_client();

    let res = client
        .post("https://ely.by/skins/wear")
        .header("Cookie", session.cookie_header())
        .header("User-Agent", USER_AGENT)
        .header("Referer", "https://ely.by/skins")
        .header("X-Requested-With", "XMLHttpRequest")
        .form(&[("skinId", skin_id.to_string())])
        .send()
        .map_err(|err| err.to_string())?;
    let body = res.text().map_err(|err| err.to_string())?;
    check_session_expired(&body)?;
    Ok(body.contains("success_skin_change"))
}

#[tauri::command]
pub async fn wear_elyby_skin(skin_id: i64) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || wear_elyby_skin_blocking(skin_id))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn take_off_elyby_skin() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| wear_elyby_skin_blocking(0))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn delete_elyby_skin(skin_id: i64) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = session_or_err()?;
        let client = timed_client();

        let res = client
            .post(format!("https://ely.by/skins/remove/{skin_id}"))
            .header("Cookie", session.cookie_header())
            .header("User-Agent", USER_AGENT)
            .header("Referer", format!("https://ely.by/skins/s{skin_id}"))
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .map_err(|err| err.to_string())?;
        let body = res.text().map_err(|err| err.to_string())?;
        check_session_expired(&body)?;
        Ok(body.contains("success_skin_delete"))
    })
    .await
    .map_err(|err| err.to_string())?
}

#[derive(Deserialize)]
struct ElyUploadResponse {
    url: Option<String>,
}

#[tauri::command]
pub async fn upload_elyby_skin(file_bytes: Vec<u8>, file_name: String, name: String, is_slim: bool) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = session_or_err()?;
        let client = timed_client();

        let part = reqwest::blocking::multipart::Part::bytes(file_bytes).file_name(file_name);
        let form = reqwest::blocking::multipart::Form::new().part("file", part);

        let res = client
            .post("https://ely.by/skins/upload")
            .header("Cookie", session.cookie_header())
            .header("User-Agent", USER_AGENT)
            .header("Referer", "https://ely.by/skins/add")
            .header("X-Requested-With", "XMLHttpRequest")
            .multipart(form)
            .send()
            .map_err(|err| err.to_string())?;

        let text = res.text().map_err(|err| err.to_string())?;
        check_session_expired(&text)?;
        let body: ElyUploadResponse = serde_json::from_str(&text).map_err(|err| err.to_string())?;
        let url = body.url.ok_or("Не удалось загрузить скин на ely.by")?;

        let id_str = url
            .split("/skins/s")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .ok_or("Не удалось определить id загруженного скина")?;
        let skin_id: i64 = id_str.parse().map_err(|_| "Некорректный id скина в ответе ely.by".to_string())?;

        save_skin_metadata(&client, &session, skin_id, &name, is_slim)?;

        if let Some((_, username, _, _)) = crate::auth::load_current_session_full() {
            if let Ok(items) = fetch_elyby_skins(&client, &session, &username) {
                if let Some(item) = items.iter().find(|i| i.id == skin_id) {
                    sync_elyby_skin_to_db(&client, &username, item, &name);
                }
            }
        }

        Ok(skin_id)
    })
    .await
    .map_err(|err| err.to_string())?
}

fn save_skin_metadata(client: &reqwest::blocking::Client, session: &SkinSession, skin_id: i64, name: &str, is_slim: bool) -> Result<(), String> {
    let form = [
        ("name", name.to_string()),
        ("description", String::new()),
        ("kind", "6".to_string()),
        ("color", String::new()),
        ("tags", String::new()),
        ("isSlim", if is_slim { "1".to_string() } else { "0".to_string() }),
    ];

    let _ = client
        .post(format!("https://ely.by/skins/save/{skin_id}"))
        .header("Cookie", session.cookie_header())
        .header("User-Agent", USER_AGENT)
        .header("Referer", format!("https://ely.by/skins/s{skin_id}/edit"))
        .header("X-Requested-With", "XMLHttpRequest")
        .form(&form)
        .send()
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn urlencoding_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
