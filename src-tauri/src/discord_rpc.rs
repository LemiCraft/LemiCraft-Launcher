use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use tauri::{AppHandle, Manager};

// Discord silently drops a set_activity inside this window of the previous one (raw IPC has no queue)
const RATE_LIMIT: Duration = Duration::from_secs(15);
// While connected the current state is re-sent this often, which heals a dropped update or a Discord restart
const KEEPALIVE: Duration = Duration::from_secs(30);
// While not connected (Discord closed or still starting) — how often to try again
const RETRY: Duration = Duration::from_secs(5);

struct Desired {
    page: String,
    playing: bool,
    // Unix seconds when the current idle/playing state began — a page swap deliberately doesn't reset it
    since: i64,
    enabled: bool,
    dirty: bool,
}

// Everything blocking (connect, handshake, send) lives on the one worker thread; the public
// functions below only edit this shared state and wake it, so callers never wait on Discord
pub struct DiscordRpc {
    desired: Mutex<Desired>,
    wake: Condvar,
}

impl Default for DiscordRpc {
    fn default() -> Self {
        Self {
            desired: Mutex::new(Desired {
                page: "В лаунчере".to_string(),
                playing: false,
                since: chrono::Utc::now().timestamp(),
                enabled: false,
                dirty: true,
            }),
            wake: Condvar::new(),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn feature_on() -> bool {
    !crate::config::DISCORD_CLIENT_ID.is_empty() && crate::settings::load().discord_rpc
}

fn update(app: &AppHandle, change: impl FnOnce(&mut Desired)) {
    let state = app.state::<DiscordRpc>();
    let mut desired = lock(&state.desired);
    change(&mut desired);
    desired.dirty = true;
    drop(desired);
    state.wake.notify_all();
}

pub fn start(app: AppHandle) {
    lock(&app.state::<DiscordRpc>().desired).enabled = feature_on();
    std::thread::spawn(move || worker(app));
}

/// Active launcher page changed. Shown only while idle — a running game keeps the "playing" text.
pub fn set_page(app: &AppHandle, page: &str) {
    update(app, |d| d.page = page.to_string());
}

/// Launcher is open but no game is running (startup, game exited).
pub fn set_idle(app: &AppHandle) {
    update(app, |d| {
        d.playing = false;
        d.since = chrono::Utc::now().timestamp();
    });
}

pub fn set_playing(app: &AppHandle) {
    update(app, |d| {
        d.playing = true;
        d.since = chrono::Utc::now().timestamp();
    });
}

/// Settings were saved — only does anything if the toggle actually flipped, so saving an
/// unrelated setting (RAM slider, JVM args) doesn't spend rate-limit budget on an identical update.
pub fn refresh(app: &AppHandle) {
    let enabled = feature_on();
    let state = app.state::<DiscordRpc>();
    let mut desired = lock(&state.desired);
    if desired.enabled == enabled {
        return;
    }
    desired.enabled = enabled;
    desired.dirty = true;
    drop(desired);
    state.wake.notify_all();
}

fn worker(app: AppHandle) {
    let state = app.state::<DiscordRpc>();
    let mut client: Option<DiscordIpcClient> = None;
    let mut last_sent: Option<Instant> = None;

    loop {
        let tick = if client.is_some() { KEEPALIVE } else { RETRY };
        let _ = state.wake.wait_timeout_while(lock(&state.desired), tick, |d| !d.dirty);

        // Stay clear of Discord's window; cut short only if the feature gets switched off meanwhile
        if let Some(sent) = last_sent {
            let elapsed = sent.elapsed();
            if elapsed < RATE_LIMIT {
                let _ = state.wake.wait_timeout_while(lock(&state.desired), RATE_LIMIT - elapsed, |d| d.enabled);
            }
        }

        // Read the state only now, so everything changed while waiting collapses into this one send
        let (enabled, page, playing, since) = {
            let mut d = lock(&state.desired);
            d.dirty = false;
            (d.enabled, d.page.clone(), d.playing, d.since)
        };

        if !enabled {
            // Closing the pipe is what makes Discord drop the presence — an explicit clear_activity
            // would itself fall under the rate limit and could be silently ignored
            if let Some(mut old) = client.take() {
                let _ = old.close();
            }
            last_sent = None;
            continue;
        }

        if client.is_none() {
            let mut fresh = DiscordIpcClient::new(crate::config::DISCORD_CLIENT_ID);
            let res = fresh.connect();
            if res.is_ok() {
                client = Some(fresh);
            }
        }
        let Some(connected) = client.as_mut() else { continue };

        let details = if playing { "Играет на LemiCraft" } else { page.as_str() };
        let mut assets = activity::Assets::new()
            .large_image("logo")
            .large_text(format!("v{}", app.package_info().version));
        if let Some(name) = crate::auth::stored_username() {
            assets = assets
                .small_image(format!("{}/avatar/{name}?size=128", crate::config::API_BASE))
                .small_text(name);
        }
        let payload = activity::Activity::new()
            .details(details)
            .assets(assets)
            .buttons(vec![activity::Button::new("lemicraft.ru", crate::config::WEBSITE_URL)])
            .timestamps(activity::Timestamps::new().start(since));

        if connected.set_activity(payload).is_ok() {
            last_sent = Some(Instant::now());
        } else {
            client = None;
        }
    }
}

#[tauri::command]
pub fn set_discord_page(app: AppHandle, page: String) {
    set_page(&app, &page);
}
