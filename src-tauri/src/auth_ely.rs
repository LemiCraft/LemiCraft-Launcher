use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter};

use crate::auth::{save_account, StoredAccount};
use crate::config::API_BASE;

// The system-browser flow has no window-close signal to detect a stuck login
static LOGIN_CANCELLED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn cancel_elyby_login() {
    LOGIN_CANCELLED.store(true, Ordering::SeqCst);
}

#[derive(Deserialize)]
struct ElyAuthUrlResponse {
    #[serde(rename = "authUrl")]
    auth_url: String,
    state: String,
    #[serde(rename = "redirectUri")]
    redirect_uri: String,
}

#[derive(Deserialize)]
struct ElyExchangeResponse {
    success: bool,
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    username: Option<String>,
    uuid: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SkinInfoResponse {
    success: bool,
    #[serde(rename = "skinUrl")]
    skin_url: Option<String>,
}

#[tauri::command]
pub fn login_elyby(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        if let Err(err) = run_login(&app) {
            let _ = app.emit("auth-error", err);
        }
    });
    Ok(())
}

fn run_login(app: &AppHandle) -> Result<(), String> {
    LOGIN_CANCELLED.store(false, Ordering::SeqCst);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;

    let auth = client
        .get(format!("{API_BASE}/auth/ely/url"))
        .send()
        .map_err(|err| err.to_string())?
        .json::<ElyAuthUrlResponse>()
        .map_err(|err| err.to_string())?;

    let (port, path) = parse_redirect_uri(&auth.redirect_uri)?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|err| {
        let hint = match find_port_owner(port) {
            Some(name) => format!("Порт {port} занят приложением \"{name}\". Закройте его и попробуйте снова, либо перезагрузите компьютер"),
            None => format!("Порт {port} занят другим приложением на вашем компьютере. Закройте лишние программы (антивирус, другие лаунчеры, торрент-клиенты) или перезагрузите компьютер и попробуйте снова"),
        };
        format!("Не удалось занять порт {port} для приёма входа от ely.by: {hint} ({err})")
    })?;
    crate::shell::open_external(auth.auth_url)?;

    let (code, state) = accept_one_callback(&listener, &path, &auth.state)?;
    if state != auth.state {
        return Err("Некорректный state — возможная подмена ответа (CSRF)".to_string());
    }

    let exchange = client
        .post(format!("{API_BASE}/auth/ely/exchange"))
        .json(&serde_json::json!({ "code": code, "state": state }))
        .send()
        .map_err(|err| err.to_string())?
        .json::<ElyExchangeResponse>()
        .map_err(|err| err.to_string())?;

    if !exchange.success {
        return Err(exchange.error.unwrap_or_else(|| "Вход через ely.by не удался".to_string()));
    }
    let (access_token, username, uuid) = (
        exchange.access_token.ok_or("Ответ сервера без accessToken")?,
        exchange.username.ok_or("Ответ сервера без username")?,
        exchange.uuid.ok_or("Ответ сервера без uuid")?,
    );

    let skin_url = client
        .get(format!("{API_BASE}/skin/{username}"))
        .send()
        .ok()
        .and_then(|res| res.json::<SkinInfoResponse>().ok())
        .filter(|info| info.success)
        .and_then(|info| info.skin_url);

    let stored = StoredAccount::ElyBy { uuid, username, access_token, skin_url };
    save_account(&stored)?;

    let _ = app.emit("auth-success", crate::auth::AccountInfo::from(&stored));
    Ok(())
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn find_port_owner(port: u16) -> Option<String> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;

    let mut netstat_cmd = std::process::Command::new("netstat");
    netstat_cmd.args(["-ano", "-p", "TCP"]);
    #[cfg(windows)]
    netstat_cmd.creation_flags(CREATE_NO_WINDOW);
    let output = netstat_cmd.output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);

    let needle = format!(":{port}");
    let pid = text.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.to_uppercase().starts_with("TCP") {
            return None;
        }
        if !trimmed.to_uppercase().contains("LISTENING") {
            return None;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let local_addr = parts.get(1)?;
        if !local_addr.ends_with(&needle) {
            return None;
        }
        parts.last()?.parse::<u32>().ok()
    })?;

    let mut tasklist_cmd = std::process::Command::new("tasklist");
    tasklist_cmd.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    tasklist_cmd.creation_flags(CREATE_NO_WINDOW);
    let tasklist = tasklist_cmd.output().ok()?;
    let tasklist_text = String::from_utf8_lossy(&tasklist.stdout);
    let name = tasklist_text.lines().next()?.split(',').next()?.trim_matches('"').to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn parse_redirect_uri(redirect_uri: &str) -> Result<(u16, String), String> {
    let url = tauri::Url::parse(redirect_uri).map_err(|err| err.to_string())?;
    let port = url.port().ok_or("redirectUri без порта")?;
    Ok((port, url.path().to_string()))
}

fn accept_one_callback(listener: &TcpListener, expected_path: &str, expected_state: &str) -> Result<(String, String), String> {
    listener.set_nonblocking(true).map_err(|err| err.to_string())?;

    let deadline = Duration::from_secs(300);
    let start = std::time::Instant::now();
    loop {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                let mut reader = BufReader::new(stream.try_clone().map_err(|err| err.to_string())?);
                let mut request_line = String::new();
                reader.read_line(&mut request_line).map_err(|err| err.to_string())?;

                let raw_path = request_line.split_whitespace().nth(1).unwrap_or("");
                let full_url = format!("http://localhost{raw_path}");
                let parsed = tauri::Url::parse(&full_url).map_err(|err| err.to_string())?;

                if parsed.path() != expected_path {
                    let _ = write_response(&mut stream, 404, "Not found");
                    continue;
                }

                let params: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

                // Ignore a connection whose state doesn't match — otherwise another local
                // process racing this port could abort a real login before it arrives (DoS)
                if params.get("state").map(String::as_str) != Some(expected_state) {
                    let _ = write_response(&mut stream, 404, "Not found");
                    continue;
                }

                if let Some(err) = params.get("error") {
                    let message = params.get("error_description").cloned().unwrap_or_else(|| err.clone());
                    let _ = write_response(&mut stream, 200, &success_page(false));
                    return Err(message);
                }
                let code = params.get("code").cloned().ok_or("Ответ от ely.by без code")?;
                let state = params.get("state").cloned().ok_or("Ответ от ely.by без state")?;
                let _ = write_response(&mut stream, 200, &success_page(true));
                return Ok((code, state));
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::TimedOut => {
                if LOGIN_CANCELLED.load(Ordering::SeqCst) {
                    return Err("Вход отменён".to_string());
                }
                if start.elapsed() > deadline {
                    return Err("Истекло время ожидания входа через ely.by".to_string());
                }
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }
            Err(err) => return Err(err.to_string()),
        }
    }
}

fn write_response(stream: &mut std::net::TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let status_text = if status == 200 { "OK" } else { "Not Found" };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );
    stream.write_all(response.as_bytes())
}

fn success_page(ok: bool) -> String {
    let (title, text, icon, icon_color) = if ok {
        (
            "Готово",
            "Вход выполнен — можно закрыть эту вкладку и вернуться в лаунчер",
            r##"<circle cx="12" cy="12" r="10"/><path d="m8 12 3 3 5-6" stroke="#141317" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##,
            "#ff5f5f",
        )
    } else {
        (
            "Не удалось войти",
            "Вход через ely.by не завершён — можно закрыть эту вкладку и попробовать снова в лаунчере",
            r##"<circle cx="12" cy="12" r="10"/><path d="M9 9l6 6m0-6-6 6" stroke="#141317" stroke-width="2" fill="none" stroke-linecap="round"/>"##,
            "#ef4444",
        )
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{background:#141317;color:#f3f1f4;font-family:system-ui,sans-serif;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
         div{{text-align:center;max-width:360px;padding:24px}}svg{{margin-bottom:16px}}\
         h1{{font-size:20px;margin:0 0 8px}}p{{color:#9a95a1;margin:0}}</style>\
         </head><body><div>\
         <svg width=\"48\" height=\"48\" viewBox=\"0 0 24 24\" fill=\"{icon_color}\">{icon}</svg>\
         <h1>{title}</h1><p>{text}</p></div></body></html>"
    )
}
