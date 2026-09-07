use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

use crate::game::game_dir;

// Well-known Minecraft client id; the modern device-code endpoint rejects it (AADSTS700016),
// so this goes through the legacy login.live.com authorization-code flow instead
const CLIENT_ID: &str = "00000000402b5328";
const REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";
const SCOPE: &str = "XboxLive.signin offline_access";

fn timed_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

fn account_path() -> PathBuf {
    game_dir().join("account.sec")
}

fn legacy_account_path() -> PathBuf {
    game_dir().join("account.json")
}

// Old C# launcher's files — resolve here once game_dir() points at the real shared folder
fn old_launcher_profile_path() -> PathBuf {
    game_dir().join("user.sec")
}
fn old_launcher_plain_profile_path() -> PathBuf {
    game_dir().join("user.json")
}

#[cfg(windows)]
pub(crate) fn encrypt_bytes(data: &[u8]) -> Vec<u8> {
    crate::dpapi::protect(data).unwrap_or_else(|_| data.to_vec())
}
#[cfg(not(windows))]
pub(crate) fn encrypt_bytes(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

#[cfg(windows)]
pub(crate) fn decrypt_bytes(data: &[u8]) -> Option<Vec<u8>> {
    crate::dpapi::unprotect(data).ok()
}
#[cfg(not(windows))]
pub(crate) fn decrypt_bytes(data: &[u8]) -> Option<Vec<u8>> {
    Some(data.to_vec())
}

// Only for reading the old C# launcher's user.sec — it predates our entropy-bound scheme above
#[cfg(windows)]
fn decrypt_legacy_bytes(data: &[u8]) -> Option<Vec<u8>> {
    crate::dpapi::unprotect_legacy(data).ok()
}
#[cfg(not(windows))]
fn decrypt_legacy_bytes(data: &[u8]) -> Option<Vec<u8>> {
    Some(data.to_vec())
}

#[derive(Deserialize)]
struct OldUserProfile {
    #[serde(rename = "Username")]
    username: String,
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "Uuid")]
    uuid: String,
    #[serde(rename = "Provider")]
    provider: String,
}

// Microsoft profiles carry no refresh_token in the old schema, so only Ely.by migrates
fn load_old_launcher_account() -> Option<StoredAccount> {
    let text = fs::read(old_launcher_profile_path())
        .ok()
        .and_then(|bytes| decrypt_legacy_bytes(&bytes))
        .and_then(|plain| String::from_utf8(plain).ok())
        .or_else(|| fs::read_to_string(old_launcher_plain_profile_path()).ok())?;

    let old: OldUserProfile = serde_json::from_str(&text).ok()?;
    if old.provider != "Ely.by" || old.access_token.is_empty() {
        return None;
    }
    Some(StoredAccount::ElyBy { uuid: old.uuid, username: old.username, access_token: old.access_token, skin_url: None })
}

fn load_stored_account() -> Option<StoredAccount> {
    if let Some(account) = fs::read(account_path())
        .ok()
        .and_then(|bytes| decrypt_bytes(&bytes))
        .and_then(|plain| String::from_utf8(plain).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        return Some(account);
    }

    if let Ok(text) = fs::read_to_string(legacy_account_path()) {
        if let Ok(account) = serde_json::from_str::<StoredAccount>(&text) {
            let _ = save_account(&account);
            return Some(account);
        }
    }

    if let Some(account) = load_old_launcher_account() {
        let _ = save_account(&account);
        let _ = fs::remove_file(old_launcher_profile_path());
        let _ = fs::remove_file(old_launcher_plain_profile_path());
        return Some(account);
    }

    None
}

fn validate_elyby_token(access_token: &str) -> Option<bool> {
    let client = timed_client();
    let res = client
        .post("https://authserver.ely.by/auth/validate")
        .json(&serde_json::json!({ "accessToken": access_token }))
        .send()
        .ok()?;
    let status = res.status();
    if status.is_success() {
        Some(true)
    } else if status.as_u16() == 401 || status.as_u16() == 400 {
        Some(false)
    } else {
        None
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "provider")]
pub(crate) enum StoredAccount {
    Microsoft {
        uuid: String,
        username: String,
        access_token: String,
        refresh_token: Option<String>,
        expires_at: u64,
        #[serde(default)]
        skin_url: Option<String>,
    },
    ElyBy {
        uuid: String,
        username: String,
        access_token: String,
        #[serde(default)]
        skin_url: Option<String>,
    },
}

impl StoredAccount {
    fn uuid(&self) -> &str {
        match self {
            Self::Microsoft { uuid, .. } | Self::ElyBy { uuid, .. } => uuid,
        }
    }
    fn username(&self) -> &str {
        match self {
            Self::Microsoft { username, .. } | Self::ElyBy { username, .. } => username,
        }
    }
    fn access_token(&self) -> &str {
        match self {
            Self::Microsoft { access_token, .. } | Self::ElyBy { access_token, .. } => access_token,
        }
    }
    fn skin_url(&self) -> Option<String> {
        match self {
            Self::Microsoft { skin_url, .. } | Self::ElyBy { skin_url, .. } => skin_url.clone(),
        }
    }
    fn provider_label(&self) -> &'static str {
        match self {
            Self::Microsoft { .. } => "Microsoft",
            Self::ElyBy { .. } => "Ely.by",
        }
    }
}

pub(crate) fn save_account(account: &StoredAccount) -> Result<(), String> {
    fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(account).unwrap();
    fs::write(account_path(), encrypt_bytes(json.as_bytes())).map_err(|err| err.to_string())?;
    let _ = fs::remove_file(legacy_account_path());
    Ok(())
}

#[derive(Serialize, Clone)]
pub struct AccountInfo {
    uuid: String,
    username: String,
    skin_url: Option<String>,
    provider: String,
}

impl From<&StoredAccount> for AccountInfo {
    fn from(a: &StoredAccount) -> Self {
        Self {
            uuid: a.uuid().to_string(),
            username: a.username().to_string(),
            skin_url: a.skin_url(),
            provider: a.provider_label().to_string(),
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn authorize_url() -> Url {
    let mut url = Url::parse("https://login.live.com/oauth20_authorize.srf").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("response_mode", "query")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPE)
        .append_pair("prompt", "select_account");
    url
}

fn extract_query_param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned())
}

fn get_authorization_code(app: &AppHandle) -> Result<String, String> {
    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    let tx_nav = tx.clone();

    let window = WebviewWindowBuilder::new(app, "msa-login", WebviewUrl::External(authorize_url()))
        .title("Вход через Microsoft")
        .inner_size(480.0, 640.0)
        .center()
        .on_navigation(move |url| {
            if url.as_str().starts_with(REDIRECT_URI) {
                if let Some(code) = extract_query_param(url, "code") {
                    let _ = tx_nav.send(Ok(code));
                } else {
                    let err = extract_query_param(url, "error_description")
                        .or_else(|| extract_query_param(url, "error"))
                        .unwrap_or_else(|| "Вход не завершён".to_string());
                    let _ = tx_nav.send(Err(err));
                }
                return false;
            }
            true
        })
        .build()
        .map_err(|err| err.to_string())?;

    let tx_close = tx.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = tx_close.send(Err("Окно входа закрыто".to_string()));
        }
    });

    let result = rx.recv_timeout(Duration::from_secs(300)).map_err(|_| "Истекло время ожидания входа".to_string())?;
    if let Some(w) = app.get_webview_window("msa-login") {
        let _ = w.close();
    }
    result
}

#[derive(Deserialize)]
struct LiveTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

fn exchange_code_for_token(client: &reqwest::blocking::Client, code: &str) -> Result<LiveTokenResponse, String> {
    let params = [
        ("client_id", CLIENT_ID),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", REDIRECT_URI),
        ("scope", SCOPE),
    ];
    request_token(client, &params)
}

fn refresh_live_token(client: &reqwest::blocking::Client, refresh_token: &str) -> Result<LiveTokenResponse, String> {
    let params = [
        ("client_id", CLIENT_ID),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
        ("scope", SCOPE),
    ];
    request_token(client, &params)
}

fn request_token(client: &reqwest::blocking::Client, params: &[(&str, &str)]) -> Result<LiveTokenResponse, String> {
    let res = client
        .post("https://login.live.com/oauth20_token.srf")
        .form(params)
        .send()
        .map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().unwrap_or_default();
        return Err(format!("Microsoft token exchange failed ({status}): {body}"));
    }
    res.json().map_err(|err| err.to_string())
}

#[derive(Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<XuiEntry>,
}

#[derive(Deserialize)]
struct XuiEntry {
    uhs: String,
}

fn xbl_authenticate(client: &reqwest::blocking::Client, ms_access_token: &str) -> Result<XblResponse, String> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    });
    let res = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&body)
        .send()
        .map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().unwrap_or_default();
        return Err(format!("Xbox Live auth failed ({status}): {text}"));
    }
    res.json().map_err(|err| err.to_string())
}

fn xsts_authorize(client: &reqwest::blocking::Client, xbl_token: &str) -> Result<XblResponse, String> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token],
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });
    let res = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&body)
        .send()
        .map_err(|err| err.to_string())?;
    let status = res.status();
    if status.as_u16() == 401 {
        return Err(
            "Этот Microsoft-аккаунт не подходит для Xbox/Minecraft (например, детский аккаунт без семьи, или регион без Xbox Live)".to_string(),
        );
    }
    if !status.is_success() {
        let text = res.text().unwrap_or_default();
        return Err(format!("XSTS authorize failed ({status}): {text}"));
    }
    res.json().map_err(|err| err.to_string())
}

fn minecraft_login(client: &reqwest::blocking::Client, uhs: &str, xsts_token: &str) -> Result<String, String> {
    let identity_token = format!("XBL3.0 x={uhs};{xsts_token}");
    let body = serde_json::json!({ "identityToken": identity_token });
    let res = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&body)
        .send()
        .map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().unwrap_or_default();
        return Err(format!("Minecraft login failed ({status}): {text}"));
    }
    #[derive(Deserialize)]
    struct McAuthResponse {
        access_token: String,
    }
    let parsed: McAuthResponse = res.json().map_err(|err| err.to_string())?;
    Ok(parsed.access_token)
}

#[derive(Deserialize)]
struct McProfile {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<McSkin>,
}

#[derive(Deserialize)]
struct McSkin {
    url: String,
    state: String,
}

impl McProfile {
    fn active_skin_url(&self) -> Option<String> {
        self.skins
            .iter()
            .find(|skin| skin.state == "ACTIVE")
            .or_else(|| self.skins.first())
            .map(|skin| skin.url.clone())
    }
}

fn minecraft_profile(client: &reqwest::blocking::Client, mc_access_token: &str) -> Result<McProfile, String> {
    let res = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_access_token)
        .send()
        .map_err(|err| err.to_string())?;
    if res.status().as_u16() == 404 {
        return Err("У этого аккаунта нет купленной лицензии Minecraft".to_string());
    }
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().unwrap_or_default();
        return Err(format!("Не удалось получить профиль Minecraft ({status}): {text}"));
    }
    res.json().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn login_microsoft(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        if let Err(err) = run_login(&app) {
            let _ = app.emit("auth-error", err);
        }
    });
    Ok(())
}

fn run_login(app: &AppHandle) -> Result<(), String> {
    let code = get_authorization_code(app)?;

    let client = timed_client();
    let token = exchange_code_for_token(&client, &code)?;
    let xbl = xbl_authenticate(&client, &token.access_token)?;
    let uhs = xbl
        .display_claims
        .xui
        .first()
        .ok_or("Xbox Live ответ без uhs")?
        .uhs
        .clone();
    let xsts = xsts_authorize(&client, &xbl.token)?;
    let mc_access_token = minecraft_login(&client, &uhs, &xsts.token)?;
    let profile = minecraft_profile(&client, &mc_access_token)?;

    let stored = StoredAccount::Microsoft {
        uuid: profile.id.clone(),
        username: profile.name.clone(),
        access_token: mc_access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        skin_url: profile.active_skin_url(),
    };
    save_account(&stored)?;

    let _ = app.emit("auth-success", AccountInfo::from(&stored));
    Ok(())
}

// fetch_skin_url gates an extra profile call callers that only need uuid/username/token can skip
fn load_current_account(fetch_skin_url: bool) -> Option<StoredAccount> {
    let stored = load_stored_account()?;

    let StoredAccount::Microsoft { uuid, username, access_token, refresh_token, expires_at, skin_url } = stored
    else {
        if fetch_skin_url {
            if let StoredAccount::ElyBy { access_token, .. } = &stored {
                if validate_elyby_token(access_token) == Some(false) {
                    let _ = fs::remove_file(account_path());
                    return None;
                }
            }
        }
        return Some(stored);
    };
    let client = timed_client();

    if now_unix() + 60 < expires_at {
        let skin_url = if fetch_skin_url && skin_url.is_none() {
            minecraft_profile(&client, &access_token).ok().and_then(|p| p.active_skin_url())
        } else {
            skin_url
        };
        let refreshed = StoredAccount::Microsoft { uuid, username, access_token, refresh_token, expires_at, skin_url };
        // A failed persist here is not harmless — the next launch would still hold the old,
        // by-then-already-consumed refresh_token and be forced into an unexplained re-login
        if let Err(err) = save_account(&refreshed) {
            log::warn!("failed to persist refreshed Microsoft account: {err}");
        }
        return Some(refreshed);
    }

    let refresh_token = refresh_token?;
    let token = refresh_live_token(&client, &refresh_token).ok()?;
    let xbl = xbl_authenticate(&client, &token.access_token).ok()?;
    let uhs = xbl.display_claims.xui.first()?.uhs.clone();
    let xsts = xsts_authorize(&client, &xbl.token).ok()?;
    let mc_access_token = minecraft_login(&client, &uhs, &xsts.token).ok()?;
    let skin_url = if fetch_skin_url {
        minecraft_profile(&client, &mc_access_token).ok().and_then(|p| p.active_skin_url())
    } else {
        skin_url
    };

    let refreshed = StoredAccount::Microsoft {
        uuid,
        username,
        access_token: mc_access_token,
        refresh_token: token.refresh_token.or(Some(refresh_token)),
        expires_at: now_unix() + token.expires_in,
        skin_url,
    };
    if let Err(err) = save_account(&refreshed) {
        log::warn!("failed to persist refreshed Microsoft account: {err}");
    }

    Some(refreshed)
}

// Crate-internal — carries the raw token, unlike AccountInfo which the frontend sees
pub(crate) fn load_current_session_full() -> Option<(String, String, String, String)> {
    load_current_account(false)
        .map(|a| (a.uuid().to_string(), a.username().to_string(), a.access_token().to_string(), a.provider_label().to_string()))
}

#[tauri::command]
pub async fn get_current_account() -> Option<AccountInfo> {
    tauri::async_runtime::spawn_blocking(|| load_current_account(true).as_ref().map(AccountInfo::from))
        .await
        .unwrap_or(None)
}

#[tauri::command]
pub fn logout() -> Result<(), String> {
    fs::remove_file(account_path()).ok();
    fs::remove_file(legacy_account_path()).ok();
    Ok(())
}
