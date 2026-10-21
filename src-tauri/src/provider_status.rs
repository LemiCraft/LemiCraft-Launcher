// Is the service behind the current account (Mojang or Ely.by) reachable? A down Ely.by kills the game at startup with an
// empty game log, so the player gets a banner and the problems go to the logs too

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const PROBE_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct ProviderStatus {
    provider: String,
    ok: bool,
    problems: Vec<String>,
}

// already written to the logs, so a service that stays down isn't repeated on every check
static REPORTED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn probes(provider: &str) -> &'static [(&'static str, &'static str)] {
    match provider {
        // Only what a game start and a login need; the Ely.by website (skins page) can be down while these work
        "Ely.by" => &[
            ("Сервер входа Ely.by", crate::config::ELYBY_AUTHLIB_API),
            ("Аккаунты Ely.by", "https://account.ely.by/oauth2/v1"),
        ],
        _ => &[
            ("Mojang (вход и профиль)", "https://api.minecraftservices.com/minecraft/profile"),
            ("Mojang (сессии)", "https://sessionserver.mojang.com/session/minecraft/profile/00000000000000000000000000000000"),
        ],
    }
}

// Alive means it answered with anything but a 5xx (an unauthenticated 401 or 404 is a healthy server)
fn probe(client: &reqwest::blocking::Client, label: &str, url: &str) -> Option<String> {
    match client.get(url).header("User-Agent", "LemiCraft-Launcher").send() {
        Ok(res) if res.status().is_server_error() => Some(format!("{label} не отвечает (код {})", res.status().as_u16())),
        Ok(_) => None,
        Err(err) if err.is_timeout() => Some(format!("{label} не ответил вовремя")),
        Err(_) => Some(format!("{label} недоступен (нет соединения)")),
    }
}

fn check(provider: &str, targets: &[(&str, &str)]) -> ProviderStatus {
    let client = reqwest::blocking::Client::builder().timeout(PROBE_TIMEOUT).build().unwrap_or_default();
    let problems: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = targets.iter().map(|(label, url)| scope.spawn(|| probe(&client, label, url))).collect();
        handles.into_iter().filter_map(|handle| handle.join().ok().flatten()).collect()
    });
    ProviderStatus { provider: provider.to_string(), ok: problems.is_empty(), problems }
}

// Blocking; tells the frontend and logs the problems when they are new
pub fn check_and_publish(app: &AppHandle) -> ProviderStatus {
    let status = match crate::auth::stored_provider() {
        Some(provider) => check(provider, probes(provider)),
        None => ProviderStatus { provider: String::new(), ok: true, problems: Vec::new() },
    };

    let changed = {
        let mut reported = REPORTED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let changed = *reported != status.problems;
        *reported = status.problems.clone();
        changed
    };
    if changed {
        for problem in &status.problems {
            log::warn!("{problem}");
            crate::game::push_log(app, &format!("[LemiCraft] {problem}"));
        }
    }
    let _ = app.emit("provider-status", status.clone());
    status
}

// the log is emptied when a game starts, so what is still wrong is written again
pub fn replay_into_log(app: &AppHandle) {
    let reported = REPORTED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    for problem in reported {
        crate::game::push_log(app, &format!("[LemiCraft] {problem}"));
    }
}

#[tauri::command]
pub async fn check_provider_status(app: AppHandle) -> ProviderStatus {
    tauri::async_runtime::spawn_blocking(move || check_and_publish(&app))
        .await
        .unwrap_or(ProviderStatus { provider: String::new(), ok: true, problems: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn serve_once(status: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        });
        url
    }

    fn client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build().unwrap()
    }

    #[test]
    fn an_answer_that_is_not_a_5xx_means_the_service_is_alive() {
        for status in ["200 OK", "204 No Content", "401 Unauthorized", "404 Not Found"] {
            assert_eq!(probe(&client(), "Сервис", &serve_once(status)), None, "{status}");
        }
    }

    #[test]
    fn a_5xx_names_the_service_and_the_code() {
        assert_eq!(probe(&client(), "Ely.by", &serve_once("503 Service Unavailable")).as_deref(), Some("Ely.by не отвечает (код 503)"));
    }

    #[test]
    fn no_connection_is_reported_as_unreachable() {
        assert_eq!(probe(&client(), "Ely.by", "http://127.0.0.1:9/").as_deref(), Some("Ely.by недоступен (нет соединения)"));
    }

    #[test]
    fn a_status_collects_every_failed_probe_and_stays_ok_without_any() {
        let down = serve_once("503 Service Unavailable");
        let up = serve_once("200 OK");
        let status = check("Ely.by", &[("Первый", &down), ("Второй", &up), ("Третий", "http://127.0.0.1:9/")]);
        assert!(!status.ok);
        assert_eq!(status.problems, vec!["Первый не отвечает (код 503)".to_string(), "Третий недоступен (нет соединения)".to_string()]);
        let healthy = check("Ely.by", &[("Живой", &serve_once("200 OK"))]);
        assert_eq!(healthy, ProviderStatus { provider: "Ely.by".into(), ok: true, problems: Vec::new() });
    }

    #[test]
    fn each_provider_has_its_own_probes() {
        assert!(probes("Ely.by").iter().any(|(_, url)| url.contains("ely.by")));
        assert!(probes("Microsoft").iter().any(|(_, url)| url.contains("minecraftservices")));
    }
}
