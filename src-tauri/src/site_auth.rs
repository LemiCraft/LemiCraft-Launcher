// Identity proof for the LemiCraft API: the provider token is exchanged for a short-lived site token (memory only,
// sent as a Bearer header). Until the site requires it, any failure here must leave the request as it was: no header

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::HeaderValue;
use serde::Deserialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const RENEW_MARGIN: Duration = Duration::from_secs(60);
// a failed exchange isn't retried on every request
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(60);
const DEFAULT_LIFETIME: Duration = Duration::from_secs(300);

const LOGIN_AGAIN: &str = "Войдите в аккаунт заново";
const OTHER_NICK: &str = "Аккаунт в лаунчере не совпадает с ником, для которого вы меняете скин. Выйдите из аккаунта и войдите снова";
const TOO_MANY: &str = "Слишком много запросов, подождите минуту и повторите";
const UNCONFIRMED: &str = "Сайт сейчас не может подтвердить аккаунт, попробуйте позже";

#[derive(Clone)]
pub(crate) struct Session {
    pub uuid: String,
    // the provider's token, not the site's
    pub token: String,
    pub provider: String,
}

impl Session {
    fn wire_provider(&self) -> &'static str {
        if self.provider == "Ely.by" { "elyby" } else { "microsoft" }
    }

    fn key(&self) -> String {
        format!("{}:{}", self.wire_provider(), self.uuid)
    }
}

pub(crate) trait Sessions {
    fn current(&self) -> Option<Session>;
    // refresh now even if not expired
    fn force_refreshed(&self) -> Option<Session>;
}

struct Accounts;

fn to_session((uuid, _username, token, provider): (String, String, String, String)) -> Session {
    Session { uuid, token, provider }
}

impl Sessions for Accounts {
    fn current(&self) -> Option<Session> {
        crate::auth::load_current_session_full().map(to_session)
    }
    fn force_refreshed(&self) -> Option<Session> {
        crate::auth::force_refresh_current_session().map(to_session)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Failure {
    // provider token refused: log in again
    Rejected,
    // no endpoint yet, rate limit, provider down, no network
    Unavailable,
}

enum Standing {
    Token(String),
    Failed(Failure),
    NoSession,
}

struct Cached {
    key: String,
    token: String,
    expires_at: Instant,
}

#[derive(Default)]
pub(crate) struct SiteAuth {
    cached: Mutex<Option<Cached>>,
    failed: Mutex<Option<(String, Failure, Instant)>>,
}

static GLOBAL: SiteAuth = SiteAuth { cached: Mutex::new(None), failed: Mutex::new(None) };

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    success: bool,
    token: Option<String>,
    #[serde(rename = "expiresIn")]
    expires_in: Option<u64>,
}

// Dropped on logout so the next account never reuses the previous one's token
pub(crate) fn clear() {
    GLOBAL.clear();
}

pub(crate) fn send_authed(client: &Client, build: impl Fn() -> RequestBuilder) -> Result<Response, String> {
    GLOBAL.send_with(client, crate::config::API_BASE, &Accounts, build)
}

fn with_headers(request: RequestBuilder, token: Option<&str>) -> RequestBuilder {
    let request = request.header("X-Launcher-Version", VERSION);
    let Some(token) = token else { return request };
    match HeaderValue::from_str(&format!("Bearer {token}")) {
        Ok(mut value) => {
            // hidden even if the request is ever printed
            value.set_sensitive(true);
            request.header(reqwest::header::AUTHORIZATION, value)
        }
        Err(_) => request,
    }
}

impl SiteAuth {
    fn clear(&self) {
        *lock(&self.cached) = None;
        *lock(&self.failed) = None;
    }

    fn cached_token(&self, key: &str) -> Option<String> {
        let guard = lock(&self.cached);
        let cached = guard.as_ref()?;
        (cached.key == key && Instant::now() + RENEW_MARGIN < cached.expires_at).then(|| cached.token.clone())
    }

    fn remember(&self, key: String, token: String, lifetime: Duration) {
        *lock(&self.cached) = Some(Cached { key, token, expires_at: Instant::now() + lifetime });
        *lock(&self.failed) = None;
    }

    fn recent_failure(&self, key: &str) -> Option<Failure> {
        let guard = lock(&self.failed);
        let (failed_key, failure, at) = guard.as_ref()?;
        (failed_key == key && at.elapsed() < RETRY_AFTER_FAILURE).then_some(*failure)
    }

    fn exchange(&self, client: &Client, base: &str, session: &Session) -> Result<(String, Duration), Failure> {
        let body = serde_json::json!({ "provider": session.wire_provider(), "accessToken": session.token });
        let res = client
            .post(format!("{base}/launcher/auth/token"))
            .header("X-Launcher-Version", VERSION)
            .json(&body)
            .send()
            .map_err(|err| {
                log::warn!("site token exchange failed: {}", err.without_url());
                Failure::Unavailable
            })?;
        match res.status().as_u16() {
            200 => {
                let parsed: TokenResponse = res.json().map_err(|_| Failure::Unavailable)?;
                match parsed.token.filter(|t| parsed.success && !t.is_empty()) {
                    Some(token) => Ok((token, parsed.expires_in.map(Duration::from_secs).unwrap_or(DEFAULT_LIFETIME))),
                    None => Err(Failure::Unavailable),
                }
            }
            401 => Err(Failure::Rejected),
            status => {
                log::warn!("site token exchange answered {status}");
                Err(Failure::Unavailable)
            }
        }
    }

    // `fresh` drops the held token first (after a 401)
    fn site_token(&self, client: &Client, base: &str, sessions: &dyn Sessions, session: &Session, fresh: bool) -> Standing {
        let key = session.key();
        if fresh {
            *lock(&self.cached) = None;
        } else if let Some(token) = self.cached_token(&key) {
            return Standing::Token(token);
        }
        if let Some(failure) = self.recent_failure(&key) {
            return Standing::Failed(failure);
        }

        let result = match self.exchange(client, base, session) {
            Err(Failure::Rejected) => match sessions.force_refreshed() {
                // maybe just stale: renew once, exchange again
                Some(renewed) => self.exchange(client, base, &renewed).map(|ok| (renewed.key(), ok)),
                None => Err(Failure::Rejected),
            },
            other => other.map(|ok| (key.clone(), ok)),
        };
        match result {
            Ok((key, (token, lifetime))) => {
                self.remember(key, token.clone(), lifetime);
                Standing::Token(token)
            }
            Err(failure) => {
                *lock(&self.failed) = Some((key, failure, Instant::now()));
                Standing::Failed(failure)
            }
        }
    }

    fn send_with(
        &self,
        client: &Client,
        base: &str,
        sessions: &dyn Sessions,
        build: impl Fn() -> RequestBuilder,
    ) -> Result<Response, String> {
        let session = sessions.current();
        let mut standing = match &session {
            Some(session) => self.site_token(client, base, sessions, session, false),
            None => Standing::NoSession,
        };
        let send = |standing: &Standing| {
            let token = if let Standing::Token(token) = standing { Some(token.as_str()) } else { None };
            with_headers(build(), token).send().map_err(|err| crate::auth::net_error("LemiCraft", &err))
        };

        let mut res = send(&standing)?;
        if res.status().as_u16() == 401 {
            if let Some(session) = &session {
                standing = self.site_token(client, base, sessions, session, true);
                if matches!(standing, Standing::Token(_)) {
                    res = send(&standing)?;
                }
            }
        }

        match res.status().as_u16() {
            401 if matches!(standing, Standing::Failed(Failure::Unavailable)) => Err(UNCONFIRMED.to_string()),
            401 => Err(LOGIN_AGAIN.to_string()),
            403 => Err(OTHER_NICK.to_string()),
            429 => Err(too_many_message(res)),
            _ => Ok(res),
        }
    }
}

// the site puts the wait in the text ("... через N сек."), there is no Retry-After
fn too_many_message(res: Response) -> String {
    let wait = res
        .text()
        .ok()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|json| json.get("message")?.as_str().map(str::to_string))
        .and_then(|message| message.split_whitespace().find_map(|word| word.parse::<u64>().ok()));
    match wait {
        Some(seconds) => format!("Слишком много запросов, повторите через {seconds} сек"),
        None => TOO_MANY.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug)]
    struct Recorded {
        target: String,
        headers: HashMap<String, String>,
        body: String,
    }

    // local server: a queue of canned answers per path (the last repeats), every request recorded
    struct Mock {
        base: String,
        seen: Arc<Mutex<Vec<Recorded>>>,
    }

    impl Mock {
        fn start(routes: Vec<(&str, Vec<(u16, &str)>)>) -> Mock {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let seen = Arc::new(Mutex::new(Vec::new()));
            let mut queues: HashMap<String, VecDeque<(u16, String)>> = routes
                .into_iter()
                .map(|(path, replies)| (path.to_string(), replies.into_iter().map(|(code, body)| (code, body.to_string())).collect()))
                .collect();
            let recorded = seen.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let mut raw = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let (head, mut body) = loop {
                        let n = stream.read(&mut chunk).unwrap_or(0);
                        if n == 0 {
                            break (String::from_utf8_lossy(&raw).to_string(), Vec::new());
                        }
                        raw.extend_from_slice(&chunk[..n]);
                        if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                            break (String::from_utf8_lossy(&raw[..pos]).to_string(), raw[pos + 4..].to_vec());
                        }
                    };
                    let mut lines = head.lines();
                    let first = lines.next().unwrap_or("");
                    let mut parts = first.split(' ');
                    let _method = parts.next();
                    let target = parts.next().unwrap_or("").to_string();
                    let headers: HashMap<String, String> = lines
                        .filter_map(|l| l.split_once(':'))
                        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
                        .collect();
                    let wanted: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                    while body.len() < wanted {
                        let n = stream.read(&mut chunk).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        body.extend_from_slice(&chunk[..n]);
                    }
                    recorded.lock().unwrap().push(Recorded {
                        target: target.clone(),
                        headers,
                        body: String::from_utf8_lossy(&body).to_string(),
                    });
                    let path = target.split('?').next().unwrap_or("").to_string();
                    let (code, text) = match queues.get_mut(&path) {
                        Some(queue) if queue.len() > 1 => queue.pop_front().unwrap(),
                        Some(queue) => queue.front().cloned().unwrap_or((404, String::new())),
                        None => (404, String::new()),
                    };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    );
                }
            });
            Mock { base, seen }
        }

        fn hits(&self, path: &str) -> Vec<Recorded> {
            self.seen.lock().unwrap().iter().filter(|r| r.target.split('?').next() == Some(path)).cloned().collect()
        }
    }

    struct FakeSessions {
        current: Option<Session>,
        renewed: Option<Session>,
        renewals: Mutex<u32>,
    }

    impl Sessions for FakeSessions {
        fn current(&self) -> Option<Session> {
            self.current.clone()
        }
        fn force_refreshed(&self) -> Option<Session> {
            *self.renewals.lock().unwrap() += 1;
            self.renewed.clone()
        }
    }

    fn session(token: &str) -> Session {
        Session { uuid: "u-1".into(), token: token.into(), provider: "Microsoft".into() }
    }

    fn sessions(current: Option<Session>, renewed: Option<Session>) -> FakeSessions {
        FakeSessions { current, renewed, renewals: Mutex::new(0) }
    }

    const TOKEN_OK: &str = r#"{"success":true,"token":"site-token-1","expiresIn":3600,"uuid":"u-1","username":"Steve"}"#;
    const TOKEN_OK_2: &str = r#"{"success":true,"token":"site-token-2","expiresIn":3600}"#;
    const ERR: &str = r#"{"success":false,"message":"nope"}"#;
    const DONE: &str = r#"{"success":true}"#;

    fn run(auth: &SiteAuth, mock: &Mock, sessions: &FakeSessions) -> Result<(u16, String), String> {
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
        let url = format!("{}/launcher/skins/apply", mock.base);
        let res = auth.send_with(&client, &mock.base, sessions, || client.post(&url).json(&serde_json::json!({ "skinId": 1, "username": "Steve" })))?;
        let status = res.status().as_u16();
        Ok((status, res.text().unwrap_or_default()))
    }

    fn bearer(hit: &Recorded) -> Option<String> {
        hit.headers.get("authorization").cloned()
    }

    #[test]
    fn exchanged_token_goes_in_the_header_and_is_reused() {
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, TOKEN_OK)]), ("/launcher/skins/apply", vec![(200, DONE)])]);
        let auth = SiteAuth::default();
        let s = sessions(Some(session("provider-token-secret")), None);
        assert_eq!(run(&auth, &mock, &s).unwrap().0, 200);
        assert_eq!(run(&auth, &mock, &s).unwrap().0, 200);

        let exchanges = mock.hits("/launcher/auth/token");
        assert_eq!(exchanges.len(), 1, "the second call must reuse the held token");
        assert!(exchanges[0].body.contains(r#""provider":"microsoft""#) && exchanges[0].body.contains("provider-token-secret"));
        for hit in mock.hits("/launcher/skins/apply") {
            assert_eq!(bearer(&hit).as_deref(), Some("Bearer site-token-1"));
            assert_eq!(hit.headers.get("x-launcher-version").map(String::as_str), Some(VERSION));
            assert!(!hit.target.contains("token"), "no token in the URL");
        }
    }

    #[test]
    fn elyby_account_is_exchanged_as_elyby() {
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, TOKEN_OK)]), ("/launcher/skins/apply", vec![(200, DONE)])]);
        let mut ely = session("ely-token");
        ely.provider = "Ely.by".into();
        run(&SiteAuth::default(), &mock, &sessions(Some(ely), None)).unwrap();
        assert!(mock.hits("/launcher/auth/token")[0].body.contains(r#""provider":"elyby""#));
    }

    #[test]
    fn missing_endpoint_means_the_request_goes_out_as_before() {
        // 404 on the exchange: the site isn't updated yet
        let mock = Mock::start(vec![("/launcher/skins/apply", vec![(200, DONE)])]);
        let s = sessions(Some(session("p")), None);
        let auth = SiteAuth::default();
        assert_eq!(run(&auth, &mock, &s).unwrap().0, 200);
        let hit = &mock.hits("/launcher/skins/apply")[0];
        assert_eq!(bearer(hit), None);
        assert_eq!(hit.headers.get("x-launcher-version").map(String::as_str), Some(VERSION));
        // and the failed exchange is not repeated on every request
        run(&auth, &mock, &s).unwrap();
        assert_eq!(mock.hits("/launcher/auth/token").len(), 1);
    }

    #[test]
    fn rate_limit_unavailable_provider_and_garbage_all_fall_back_to_a_plain_request() {
        for reply in [(429, ERR), (502, ERR), (500, ""), (200, "not json"), (200, ERR)] {
            let mock = Mock::start(vec![("/launcher/auth/token", vec![reply]), ("/launcher/skins/apply", vec![(200, DONE)])]);
            assert_eq!(run(&SiteAuth::default(), &mock, &sessions(Some(session("p")), None)).unwrap().0, 200, "exchange answered {reply:?}");
            assert_eq!(bearer(&mock.hits("/launcher/skins/apply")[0]), None);
        }
    }

    #[test]
    fn unreachable_exchange_falls_back_too() {
        let mock = Mock::start(vec![("/launcher/skins/apply", vec![(200, DONE)])]);
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();
        let url = format!("{}/launcher/skins/apply", mock.base);
        let res = SiteAuth::default()
            .send_with(&client, "http://127.0.0.1:9", &sessions(Some(session("p")), None), || client.post(&url))
            .unwrap();
        assert_eq!(res.status().as_u16(), 200);
        assert_eq!(bearer(&mock.hits("/launcher/skins/apply")[0]), None);
    }

    #[test]
    fn no_account_sends_the_request_without_exchanging() {
        let mock = Mock::start(vec![("/launcher/skins/apply", vec![(200, DONE)])]);
        assert_eq!(run(&SiteAuth::default(), &mock, &sessions(None, None)).unwrap().0, 200);
        assert!(mock.hits("/launcher/auth/token").is_empty());
        assert_eq!(bearer(&mock.hits("/launcher/skins/apply")[0]), None);
    }

    #[test]
    fn a_401_triggers_one_new_exchange_and_one_retry() {
        let mock = Mock::start(vec![
            ("/launcher/auth/token", vec![(200, TOKEN_OK), (200, TOKEN_OK_2)]),
            ("/launcher/skins/apply", vec![(401, ERR), (200, DONE)]),
        ]);
        let s = sessions(Some(session("p")), None);
        assert_eq!(run(&SiteAuth::default(), &mock, &s).unwrap().0, 200);
        let hits = mock.hits("/launcher/skins/apply");
        assert_eq!(hits.len(), 2);
        assert_eq!(bearer(&hits[0]).as_deref(), Some("Bearer site-token-1"));
        assert_eq!(bearer(&hits[1]).as_deref(), Some("Bearer site-token-2"));
        assert_eq!(*s.renewals.lock().unwrap(), 0, "the provider token was fine, it must not be renewed");
    }

    #[test]
    fn a_second_401_ends_with_the_login_message() {
        let mock = Mock::start(vec![
            ("/launcher/auth/token", vec![(200, TOKEN_OK), (200, TOKEN_OK_2)]),
            ("/launcher/skins/apply", vec![(401, ERR)]),
        ]);
        let err = run(&SiteAuth::default(), &mock, &sessions(Some(session("p")), None)).unwrap_err();
        assert_eq!(err, LOGIN_AGAIN);
        assert_eq!(mock.hits("/launcher/skins/apply").len(), 2, "one try and one retry, no more");
    }

    #[test]
    fn a_rejected_exchange_renews_the_provider_token_once() {
        // first exchange: 401, after renewing the provider token: success
        let mock = Mock::start(vec![
            ("/launcher/auth/token", vec![(401, ERR), (200, TOKEN_OK)]),
            ("/launcher/skins/apply", vec![(200, DONE)]),
        ]);
        let s = sessions(Some(session("stale")), Some(session("renewed")));
        assert_eq!(run(&SiteAuth::default(), &mock, &s).unwrap().0, 200);
        assert_eq!(*s.renewals.lock().unwrap(), 1);
        let exchanges = mock.hits("/launcher/auth/token");
        assert!(exchanges[0].body.contains("stale") && exchanges[1].body.contains("renewed"));
        assert_eq!(bearer(&mock.hits("/launcher/skins/apply")[0]).as_deref(), Some("Bearer site-token-1"));
    }

    #[test]
    fn an_exchange_rejected_twice_sends_plain_and_a_401_then_asks_for_a_new_login() {
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(401, ERR)]), ("/launcher/skins/apply", vec![(401, ERR)])]);
        let s = sessions(Some(session("stale")), Some(session("still-bad")));
        let err = run(&SiteAuth::default(), &mock, &s).unwrap_err();
        assert_eq!(err, LOGIN_AGAIN);
        assert_eq!(bearer(&mock.hits("/launcher/skins/apply")[0]), None);
    }

    #[test]
    fn a_401_while_the_exchange_is_down_does_not_blame_the_login() {
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(502, ERR)]), ("/launcher/skins/apply", vec![(401, ERR)])]);
        let err = run(&SiteAuth::default(), &mock, &sessions(Some(session("p")), None)).unwrap_err();
        assert_eq!(err, UNCONFIRMED);
    }

    #[test]
    fn forbidden_and_rate_limited_get_their_own_messages() {
        for (code, expected) in [(403, OTHER_NICK), (429, TOO_MANY)] {
            let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, TOKEN_OK)]), ("/launcher/skins/apply", vec![(code, ERR)])]);
            let err = run(&SiteAuth::default(), &mock, &sessions(Some(session("p")), None)).unwrap_err();
            assert_eq!(err, expected);
            assert_eq!(mock.hits("/launcher/skins/apply").len(), 1, "no retry on {code}");
        }
    }

    #[test]
    fn a_rate_limit_message_with_seconds_is_passed_on() {
        let body = r#"{"error":true,"statusCode":429,"message":"Слишком много запросов. Попробуйте через 42 сек."}"#;
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, TOKEN_OK)]), ("/launcher/skins/apply", vec![(429, body)])]);
        let err = run(&SiteAuth::default(), &mock, &sessions(Some(session("p")), None)).unwrap_err();
        assert_eq!(err, "Слишком много запросов, повторите через 42 сек");
    }

    #[test]
    fn other_statuses_are_left_to_the_caller() {
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, TOKEN_OK)]), ("/launcher/skins/apply", vec![(500, ERR)])]);
        assert_eq!(run(&SiteAuth::default(), &mock, &sessions(Some(session("p")), None)).unwrap().0, 500);
    }

    #[test]
    fn a_held_token_is_not_used_for_another_account() {
        let mock = Mock::start(vec![
            ("/launcher/auth/token", vec![(200, TOKEN_OK), (200, TOKEN_OK_2)]),
            ("/launcher/skins/apply", vec![(200, DONE)]),
        ]);
        let auth = SiteAuth::default();
        run(&auth, &mock, &sessions(Some(session("a")), None)).unwrap();
        let mut other = session("b");
        other.uuid = "u-2".into();
        run(&auth, &mock, &sessions(Some(other), None)).unwrap();
        let hits = mock.hits("/launcher/skins/apply");
        assert_eq!(bearer(&hits[0]).as_deref(), Some("Bearer site-token-1"));
        assert_eq!(bearer(&hits[1]).as_deref(), Some("Bearer site-token-2"));
    }

    #[test]
    fn a_token_about_to_expire_is_replaced_before_use() {
        // expiresIn 30 s is inside the one-minute margin, so it is never reused
        let short = r#"{"success":true,"token":"short","expiresIn":30}"#;
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, short)]), ("/launcher/skins/apply", vec![(200, DONE)])]);
        let auth = SiteAuth::default();
        let s = sessions(Some(session("p")), None);
        run(&auth, &mock, &s).unwrap();
        run(&auth, &mock, &s).unwrap();
        assert_eq!(mock.hits("/launcher/auth/token").len(), 2);
    }

    #[test]
    fn clearing_drops_the_held_token() {
        let mock = Mock::start(vec![("/launcher/auth/token", vec![(200, TOKEN_OK)]), ("/launcher/skins/apply", vec![(200, DONE)])]);
        let auth = SiteAuth::default();
        let s = sessions(Some(session("p")), None);
        run(&auth, &mock, &s).unwrap();
        auth.clear();
        run(&auth, &mock, &s).unwrap();
        assert_eq!(mock.hits("/launcher/auth/token").len(), 2);
    }

    #[test]
    fn the_authorization_header_is_marked_sensitive() {
        let client = reqwest::blocking::Client::new();
        let request = with_headers(client.get("http://127.0.0.1:1/"), Some("secret")).build().unwrap();
        let value = request.headers().get(reqwest::header::AUTHORIZATION).unwrap();
        assert!(value.is_sensitive());
        assert!(!format!("{:?}", request.headers()).contains("secret"));
    }

    #[test]
    fn elyby_refresh_returns_the_new_tokens_and_sends_the_refresh_token_in_the_body() {
        let mock = Mock::start(vec![(
            "/auth/ely/refresh",
            vec![(200, r#"{"success":true,"accessToken":"new-access","refreshToken":"new-refresh","expiresIn":86400}"#)],
        )]);
        let tokens = crate::auth::refresh_elyby_at(&mock.base, "old-refresh").unwrap();
        assert_eq!(tokens.access_token, "new-access");
        assert_eq!(tokens.refresh_token.as_deref(), Some("new-refresh"));
        assert_eq!(tokens.expires_in, Some(86400));
        let hit = &mock.hits("/auth/ely/refresh")[0];
        assert!(hit.body.contains(r#""refreshToken":"old-refresh""#));
        assert!(!hit.target.contains("old-refresh"), "no token in the URL");
    }

    #[test]
    fn elyby_refresh_answers_map_to_rejected_or_unavailable() {
        use crate::auth::ElyRefreshError;
        let cases = [
            (401, ERR, ElyRefreshError::Rejected),
            (502, ERR, ElyRefreshError::Unavailable),
            (404, "", ElyRefreshError::Unavailable),
            (200, ERR, ElyRefreshError::Unavailable),
            (200, "not json", ElyRefreshError::Unavailable),
            (200, r#"{"success":true}"#, ElyRefreshError::Unavailable),
        ];
        for (code, body, expected) in cases {
            let mock = Mock::start(vec![("/auth/ely/refresh", vec![(code, body)])]);
            assert_eq!(crate::auth::refresh_elyby_at(&mock.base, "r").err(), Some(expected), "answer {code} {body}");
        }
    }

    #[test]
    fn elyby_refresh_without_a_server_is_unavailable() {
        assert_eq!(crate::auth::refresh_elyby_at("http://127.0.0.1:9", "r").err(), Some(crate::auth::ElyRefreshError::Unavailable));
    }
}
