// Measures every way into the server (directly and through the relays) and says which is best.
// It only advises; the address the game uses is not touched here.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::time::timeout;

use crate::server_status::lookup_srv;

// A quick check asks six times, and asks six more of the ways that are close to the best or look uneven;
// a deep check asks 24 times, slowly
const QUICK_SAMPLES: usize = 6;
const EXTRA_SAMPLES: usize = 6;
const DEEP_SAMPLES: usize = 24;
const QUICK_GAP: Duration = Duration::from_millis(300);
const DEEP_GAP: Duration = Duration::from_millis(1000);
const SAMPLE_TIMEOUT: Duration = Duration::from_millis(2500);
// a name lookup that gets no answer is tried again: one lost packet must not make a way look dead
const DNS_TIMEOUT: Duration = Duration::from_secs(2);
const DNS_ATTEMPTS: usize = 2;
const SRV_TTL: Duration = Duration::from_secs(10 * 60);
// a point that fails this many times from the start is written off, so a dead relay does not hold the check up
const GIVE_UP_AFTER: usize = 3;
const DEFAULT_PORT: u16 = 25565;

const HISTORY_RUNS: usize = 12;
const HISTORY_MAX_AGE_SECS: u64 = 24 * 60 * 60;
// three quick checks' worth of answers: less than that is a first impression, not a basis for advice
const MIN_SAMPLES_FOR_ADVICE: u32 = 18;

// every 10 % of lost requests costs as much as this many ms of delay
const LOSS_COST_MS: f64 = 1000.0;
const JITTER_WEIGHT: f64 = 2.0;
// how much of the way from the usual delay to the slow tail counts
const TAIL_WEIGHT: f64 = 0.5;
// the direct way stays the advice unless another is clearly better
const BETTER_BY_RATIO: f64 = 0.75;
const BETTER_BY_MS: f64 = 20.0;
// a way within this of the best one is worth asking more of
const CONTENDER_RATIO: f64 = 1.4;

const DIRECT_ID: &str = "direct";

// Raise when the way a delay is measured changes: delays kept from the old way would be mixed with the new ones
const METRIC_VERSION: u32 = 2;

struct Point {
    id: &'static str,
    label: &'static str,
    host: &'static str,
    // where the game ends up when the domain has no SRV record (or it cannot be read)
    fallback: (&'static str, u16),
}

static POINTS: [Point; 4] = [
    Point { id: DIRECT_ID, label: "Напрямую", host: "lemicraft.ru", fallback: (crate::config::SERVER_HOST, crate::config::SERVER_PORT) },
    Point { id: "de", label: "Германия", host: "de.lemicraft.ru", fallback: ("de.lemicraft.ru", DEFAULT_PORT) },
    Point { id: "nl", label: "Нидерланды", host: "nl.lemicraft.ru", fallback: ("nl.lemicraft.ru", DEFAULT_PORT) },
    Point { id: "ru", label: "Россия (Москва)", host: "ru.lemicraft.ru", fallback: ("ru.lemicraft.ru", DEFAULT_PORT) },
];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Quick,
    Deep,
}

impl Mode {
    fn parse(value: Option<&str>) -> Mode {
        if value == Some("deep") { Mode::Deep } else { Mode::Quick }
    }

    fn samples(self) -> usize {
        match self {
            Mode::Quick => QUICK_SAMPLES,
            Mode::Deep => DEEP_SAMPLES,
        }
    }

    fn gap(self) -> Duration {
        match self {
            Mode::Quick => QUICK_GAP,
            Mode::Deep => DEEP_GAP,
        }
    }
}

/// Why a request got no delay
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProbeError {
    Refused,
    Reset,
    Timeout,
    NoPing,
    Protocol,
    Other,
}

impl ProbeError {
    fn text(self) -> &'static str {
        match self {
            ProbeError::Refused => "Сервер отклонил подключение",
            ProbeError::Reset => "Соединение оборвалось",
            ProbeError::Timeout => "Нет ответа (тайм-аут)",
            ProbeError::NoPing => "Сервер не ответил на ping",
            ProbeError::Protocol => "Неожиданный ответ сервера",
            ProbeError::Other => "Не удалось подключиться",
        }
    }
}

fn classify(err: &std::io::Error) -> ProbeError {
    use std::io::ErrorKind::*;
    match err.kind() {
        ConnectionRefused => ProbeError::Refused,
        ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof => ProbeError::Reset,
        TimedOut => ProbeError::Timeout,
        _ => ProbeError::Other,
    }
}

/// What one check of one point found: how many requests went out, how many failed, and their spread
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RunSummary {
    pub at: u64,
    pub total: u32,
    pub failed: u32,
    pub median_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    /// Fastest and slowest answer of the check
    #[serde(default)]
    pub min_ms: Option<f64>,
    #[serde(default)]
    pub max_ms: Option<f64>,
    /// The slow end of the answers (the 90th percentile): what a rare spike looks like
    #[serde(default)]
    pub tail_ms: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PointResult {
    pub id: String,
    pub label: String,
    pub host: String,
    pub address: String,
    /// Delay of the latest check (the middle one) and its fastest and slowest answer
    pub ping_ms: Option<f64>,
    #[serde(default)]
    pub min_ms: Option<f64>,
    #[serde(default)]
    pub max_ms: Option<f64>,
    /// Over the last day of checks
    pub jitter_ms: Option<f64>,
    #[serde(default)]
    pub tail_ms: Option<f64>,
    pub loss: f64,
    /// Lower is better; None when nothing ever answered
    pub score: Option<f64>,
    pub runs: usize,
    /// Requests the score rests on
    #[serde(default)]
    pub samples: u32,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Probe {
    pub points: Vec<PointResult>,
    /// Point with the best score
    pub best: Option<String>,
    /// Point worth switching to, empty when the direct way is good enough
    pub suggestion: Option<String>,
    /// A way looks clearly better, but too few answers are in to say so yet
    #[serde(default)]
    pub preliminary: bool,
    pub checked_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    metric: u32,
    runs: HashMap<String, Vec<RunSummary>>,
    last: Option<Probe>,
}

/// The history, or an empty one when it was kept by another way of measuring
fn current_or_fresh(store: Store) -> Store {
    if store.metric == METRIC_VERSION {
        store
    } else {
        Store { metric: METRIC_VERSION, ..Store::default() }
    }
}

fn sorted(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sorted = sorted(values);
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 { sorted[mid] } else { (sorted[mid - 1] + sorted[mid]) / 2.0 })
}

// mean distance from the middle value; a few samples are too few for a proper deviation
fn spread(values: &[f64]) -> Option<f64> {
    let mid = median(values)?;
    Some(values.iter().map(|v| (v - mid).abs()).sum::<f64>() / values.len() as f64)
}

// nearest rank: with six answers this is the slowest one, with 24 it is the third slowest
fn tail(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sorted = sorted(values);
    let rank = (0.9 * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

pub fn summarize(at: u64, rtts: &[Option<f64>]) -> RunSummary {
    let answered: Vec<f64> = rtts.iter().flatten().copied().collect();
    RunSummary {
        at,
        total: rtts.len() as u32,
        failed: (rtts.len() - answered.len()) as u32,
        median_ms: median(&answered),
        jitter_ms: spread(&answered),
        min_ms: answered.iter().copied().reduce(f64::min),
        max_ms: answered.iter().copied().reduce(f64::max),
        tail_ms: tail(&answered),
    }
}

#[derive(Debug, PartialEq)]
pub struct Aggregate {
    pub loss: f64,
    pub median_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub tail_ms: Option<f64>,
}

/// Several checks in a row show what one cannot: a point that is fast but drops out now and then
pub fn aggregate(history: &[RunSummary]) -> Aggregate {
    let total: u32 = history.iter().map(|r| r.total).sum();
    let failed: u32 = history.iter().map(|r| r.failed).sum();
    let medians: Vec<f64> = history.iter().filter_map(|r| r.median_ms).collect();
    let jitters: Vec<f64> = history.iter().filter_map(|r| r.jitter_ms).collect();
    let tails: Vec<f64> = history.iter().filter_map(|r| r.tail_ms).collect();
    // the jitter inside each check plus how much the checks differ from each other
    let jitter_ms = match (jitters.is_empty(), spread(&medians)) {
        (false, Some(between)) => Some(jitters.iter().sum::<f64>() / jitters.len() as f64 + between),
        _ => None,
    };
    Aggregate {
        loss: if total == 0 { 0.0 } else { failed as f64 / total as f64 },
        median_ms: median(&medians),
        jitter_ms,
        tail_ms: if tails.is_empty() { None } else { Some(tails.iter().sum::<f64>() / tails.len() as f64) },
    }
}

pub fn score(agg: &Aggregate) -> Option<f64> {
    let delay = agg.median_ms?;
    let tail_over = (agg.tail_ms.unwrap_or(delay) - delay).max(0.0);
    Some(delay + JITTER_WEIGHT * agg.jitter_ms.unwrap_or(0.0) + TAIL_WEIGHT * tail_over + LOSS_COST_MS * agg.loss)
}

#[derive(Debug, PartialEq)]
pub struct Advice {
    pub best: Option<String>,
    pub suggestion: Option<String>,
    pub preliminary: bool,
}

/// The best point, and whether another way is worth switching to. The suggestion needs a clear win over
/// the direct way and enough answers behind it; a clear win on too few answers is only called preliminary.
pub fn pick(points: &[PointResult]) -> Advice {
    let none = Advice { best: None, suggestion: None, preliminary: false };
    let best = points
        .iter()
        .filter_map(|p| p.score.map(|s| (p, s)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((best, best_score)) = best else { return none };
    if best.id == DIRECT_ID {
        return Advice { best: Some(best.id.clone()), suggestion: None, preliminary: false };
    }
    let direct = points.iter().find(|p| p.id == DIRECT_ID);
    let clearly_better = match direct.and_then(|p| p.score) {
        None => true,
        Some(d) => best_score <= d * BETTER_BY_RATIO && d - best_score >= BETTER_BY_MS,
    };
    // a direct way that never answers needs no more proof
    let direct_known = direct.is_none_or(|p| p.score.is_none() || p.samples >= MIN_SAMPLES_FOR_ADVICE);
    let enough = best.samples >= MIN_SAMPLES_FOR_ADVICE && direct_known;
    Advice {
        best: Some(best.id.clone()),
        suggestion: (clearly_better && enough).then(|| best.id.clone()),
        preliminary: clearly_better && !enough,
    }
}

/// Which points are worth asking more of after the first round: those close to the best, and those with
/// lost or very uneven answers. A point that never answered is not.
pub fn needs_more(rounds: &[Vec<Option<f64>>]) -> Vec<bool> {
    let summaries: Vec<RunSummary> = rounds.iter().map(|r| summarize(0, r)).collect();
    let scores: Vec<Option<f64>> = summaries.iter().map(|s| score(&aggregate(std::slice::from_ref(s)))).collect();
    let best = scores.iter().flatten().copied().reduce(f64::min);
    summaries
        .iter()
        .zip(&scores)
        .map(|(summary, score)| {
            let (Some(score), Some(best)) = (*score, best) else { return false };
            let uneven = matches!((summary.median_ms, summary.max_ms), (Some(m), Some(x)) if x > m * 2.0 + 20.0);
            score <= best * CONTENDER_RATIO + 10.0 || summary.failed > 0 || uneven
        })
        .collect()
}

async fn read_packet(stream: &mut tokio::net::TcpStream) -> Result<Vec<u8>, ProbeError> {
    use tokio::io::AsyncReadExt;
    let (mut len, mut shift) = (0u32, 0u32);
    loop {
        let byte = stream.read_u8().await.map_err(|err| classify(&err))?;
        len |= u32::from(byte & 0x7F) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
        if shift > 28 {
            return Err(ProbeError::Protocol);
        }
    }
    if len > 1 << 20 {
        return Err(ProbeError::Protocol);
    }
    let mut body = vec![0u8; len as usize];
    stream.read_exact(&mut body).await.map_err(|err| classify(&err))?;
    Ok(body)
}

/// One status request and one ping to an already resolved address: the round trip of the ping packet,
/// or None when the server answered the status but never echoed the ping
async fn measure_once(addr: SocketAddr, handshake_host: &str) -> Result<Option<f64>, ProbeError> {
    use crate::server_status::{write_packet, write_varint};

    let mut stream = tokio::net::TcpStream::connect(addr).await.map_err(|err| classify(&err))?;
    let _ = stream.set_nodelay(true);

    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, 754);
    write_varint(&mut handshake, handshake_host.len() as i32);
    handshake.extend_from_slice(handshake_host.as_bytes());
    handshake.extend_from_slice(&addr.port().to_be_bytes());
    write_varint(&mut handshake, 1);

    write_packet(&mut stream, &handshake).await.map_err(|_| ProbeError::Reset)?;
    write_packet(&mut stream, &[0x00]).await.map_err(|_| ProbeError::Reset)?;
    let reply = read_packet(&mut stream).await?;
    if reply.first() != Some(&0x00) {
        return Err(ProbeError::Protocol);
    }

    let mut ping = vec![0x01];
    ping.extend_from_slice(&0x4c43_i64.to_be_bytes());
    let start = Instant::now();
    if write_packet(&mut stream, &ping).await.is_err() {
        return Ok(None);
    }
    Ok(match read_packet(&mut stream).await {
        Ok(pong) if pong.first() == Some(&0x01) => Some(start.elapsed().as_secs_f64() * 1000.0),
        _ => None,
    })
}

#[derive(Default)]
struct SrvCache {
    // None inside: the domain has no SRV record
    entries: HashMap<String, (Instant, Option<(String, u16)>)>,
}

impl SrvCache {
    fn get(&self, domain: &str, now: Instant) -> Option<Option<(String, u16)>> {
        self.entries
            .get(domain)
            .filter(|(at, _)| now.duration_since(*at) < SRV_TTL)
            .map(|(_, value)| value.clone())
    }

    fn put(&mut self, domain: &str, value: Option<(String, u16)>, now: Instant) {
        self.entries.insert(domain.to_string(), (now, value));
    }
}

static SRV_CACHE: OnceLock<Mutex<SrvCache>> = OnceLock::new();

fn srv_cache() -> std::sync::MutexGuard<'static, SrvCache> {
    SRV_CACHE
        .get_or_init(|| Mutex::new(SrvCache::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn resolve(point: &Point) -> (String, u16) {
    let fallback = || (point.fallback.0.to_string(), point.fallback.1);
    let known = srv_cache().get(point.host, Instant::now());
    if let Some(known) = known {
        return known.unwrap_or_else(fallback);
    }
    for _ in 0..DNS_ATTEMPTS {
        match timeout(DNS_TIMEOUT, lookup_srv(point.host)).await {
            Ok(Ok(target)) => {
                srv_cache().put(point.host, Some(target.clone()), Instant::now());
                return target;
            }
            // answered without an SRV record: nothing more to ask, and nothing more to ask again soon
            Ok(Err(_)) => {
                srv_cache().put(point.host, None, Instant::now());
                break;
            }
            Err(_) => {}
        }
    }
    fallback()
}

// Java, which the game runs on, takes an IPv4 address first; the system's own order may put IPv6 ahead
fn prefer_ipv4(found: impl IntoIterator<Item = SocketAddr>) -> Option<SocketAddr> {
    let all: Vec<SocketAddr> = found.into_iter().collect();
    all.iter().copied().find(SocketAddr::is_ipv4).or_else(|| all.first().copied())
}

async fn lookup_address(host: &str, port: u16) -> Option<SocketAddr> {
    for _ in 0..DNS_ATTEMPTS {
        if let Ok(Ok(found)) = timeout(DNS_TIMEOUT, tokio::net::lookup_host((host, port))).await {
            return prefer_ipv4(found);
        }
    }
    None
}

/// Delays of the requests that came back, None for those that did not.
/// The delay is the ping packet's round trip, which the proxy itself answers: what the server list in
/// the game shows. The status reply is slower through a relay (it waits for the server behind it), so it
/// only has to arrive, and the TCP handshake can be answered by a local VPN or proxy.
async fn sample(
    addr: SocketAddr,
    handshake_host: &str,
    samples: usize,
    gap: Duration,
    tick: &(dyn Fn() + Sync),
) -> (Vec<Option<f64>>, Option<ProbeError>) {
    let mut rtts = Vec::with_capacity(samples);
    let mut last_error = None;
    for i in 0..samples {
        if i > 0 {
            tokio::time::sleep(gap).await;
        }
        match timeout(SAMPLE_TIMEOUT, measure_once(addr, handshake_host)).await {
            Ok(Ok(Some(ms))) => rtts.push(Some(ms)),
            Ok(Ok(None)) => {
                rtts.push(None);
                last_error = Some(ProbeError::NoPing);
            }
            Ok(Err(err)) => {
                rtts.push(None);
                last_error = Some(err);
            }
            Err(_) => {
                rtts.push(None);
                last_error = Some(ProbeError::Timeout);
            }
        }
        tick();
        if rtts.len() == GIVE_UP_AFTER && rtts.iter().all(Option::is_none) {
            break;
        }
    }
    (rtts, last_error)
}

struct PointSamples {
    address: String,
    addr: Option<SocketAddr>,
    rtts: Vec<Option<f64>>,
    error: Option<ProbeError>,
}

async fn probe_point(point: &'static Point, samples: usize, gap: Duration, tick: &(dyn Fn() + Sync)) -> PointSamples {
    let (host, port) = resolve(point).await;
    let address = format!("{host}:{port}");
    // the name is looked up once, so its lookup never counts as part of a delay
    let Some(addr) = lookup_address(&host, port).await else {
        return PointSamples { address, addr: None, rtts: vec![None; GIVE_UP_AFTER], error: None };
    };
    let (rtts, error) = sample(addr, point.host, samples, gap, tick).await;
    PointSamples { address, addr: Some(addr), rtts, error }
}

struct Progress {
    app: tauri::AppHandle,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl Progress {
    fn emit(&self) {
        let _ = self.app.emit(
            "entry-probe-progress",
            serde_json::json!({ "done": self.done.load(Ordering::Relaxed), "total": self.total.load(Ordering::Relaxed) }),
        );
    }

    fn tick(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
        self.emit();
    }

    fn add_total(&self, more: usize) {
        self.total.fetch_add(more, Ordering::Relaxed);
        self.emit();
    }

    // a point that was written off never reaches its share, the bar must still end full
    fn finish(&self) {
        self.done.store(self.total.load(Ordering::Relaxed), Ordering::Relaxed);
        self.emit();
    }
}

static RUNNING: AtomicBool = AtomicBool::new(false);

struct RunGuard;

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

// A recommendation older than this is not trusted for "auto": the network may have changed since
const AUTO_ROUTE_MAX_AGE_SECS: u64 = 3 * 24 * 60 * 60;

/// The host the auto-connect goes to for a chosen route. "auto" is the way the last check recommended if it
/// is recent, the direct way otherwise; an unknown route is the direct way too.
fn host_for_route(route: &str, last: Option<&Probe>, now: u64) -> &'static str {
    let id = if route == "auto" {
        last.filter(|probe| probe.checked_at + AUTO_ROUTE_MAX_AGE_SECS >= now).and_then(|probe| probe.suggestion.as_deref()).unwrap_or(DIRECT_ID)
    } else {
        route
    };
    POINTS.iter().find(|p| p.id == id).unwrap_or(&POINTS[0]).host
}

pub(crate) fn quick_play_host(route: &str) -> String {
    host_for_route(route, load_store().last.as_ref(), crate::auth::now_unix()).to_string()
}

#[derive(Serialize)]
pub struct RouteInfo {
    id: &'static str,
    label: &'static str,
    host: &'static str,
}

/// The ways into the server, for the choice in settings
#[tauri::command]
pub fn get_entry_routes() -> Vec<RouteInfo> {
    POINTS.iter().map(|p| RouteInfo { id: p.id, label: p.label, host: p.host }).collect()
}

fn store_path() -> std::path::PathBuf {
    crate::game::game_dir().join("entry_points.json")
}

fn load_store() -> Store {
    current_or_fresh(
        std::fs::read_to_string(store_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default(),
    )
}

fn save_store(store: &Store) {
    if let Ok(json) = serde_json::to_string(store) {
        let _ = std::fs::write(store_path(), json);
    }
}

fn trim_history(runs: &mut Vec<RunSummary>, now: u64) {
    runs.retain(|r| r.at + HISTORY_MAX_AGE_SECS >= now);
    if runs.len() > HISTORY_RUNS {
        runs.drain(..runs.len() - HISTORY_RUNS);
    }
}

#[tauri::command]
pub fn get_cached_entry_points() -> Option<Probe> {
    load_store().last
}

/// `mode` is "quick" (the default) or "deep"
#[tauri::command]
pub async fn probe_entry_points(app: tauri::AppHandle, mode: Option<String>) -> Result<Probe, String> {
    // two checks at once would both rewrite the history
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Проверка уже идёт".to_string());
    }
    let _guard = RunGuard;
    let mode = Mode::parse(mode.as_deref());
    let (samples, gap) = (mode.samples(), mode.gap());
    let progress = Arc::new(Progress { app, done: AtomicUsize::new(0), total: AtomicUsize::new(POINTS.len() * samples) });
    progress.emit();

    let handles: Vec<_> = POINTS
        .iter()
        .map(|point| {
            let progress = progress.clone();
            tauri::async_runtime::spawn(async move { probe_point(point, samples, gap, &|| progress.tick()).await })
        })
        .collect();
    let mut sampled = Vec::with_capacity(handles.len());
    for handle in handles {
        sampled.push(handle.await.map_err(|err| err.to_string())?);
    }

    if mode == Mode::Quick {
        let rounds: Vec<Vec<Option<f64>>> = sampled.iter().map(|s| s.rtts.clone()).collect();
        let more: Vec<usize> = needs_more(&rounds)
            .into_iter()
            .enumerate()
            .filter(|(i, wanted)| *wanted && sampled[*i].addr.is_some())
            .map(|(i, _)| i)
            .collect();
        if !more.is_empty() {
            progress.add_total(more.len() * EXTRA_SAMPLES);
            let handles: Vec<_> = more
                .iter()
                .filter_map(|&i| sampled[i].addr.map(|addr| (i, addr)))
                .map(|(i, addr)| {
                    let progress = progress.clone();
                    let host = POINTS[i].host;
                    (i, tauri::async_runtime::spawn(async move { sample(addr, host, EXTRA_SAMPLES, gap, &|| progress.tick()).await }))
                })
                .collect();
            for (i, handle) in handles {
                let (rtts, error) = handle.await.map_err(|err| err.to_string())?;
                sampled[i].rtts.extend(rtts);
                sampled[i].error = error.or(sampled[i].error);
            }
        }
    }
    progress.finish();

    tauri::async_runtime::spawn_blocking(move || {
        let now = crate::auth::now_unix();
        let mut store = load_store();
        let mut points = Vec::with_capacity(POINTS.len());
        for (point, found) in POINTS.iter().zip(sampled) {
            let latest = summarize(now, &found.rtts);
            let history = store.runs.entry(point.id.to_string()).or_default();
            history.push(latest.clone());
            trim_history(history, now);
            let agg = aggregate(history);
            let error = latest.median_ms.is_none().then(|| {
                match (found.addr, found.error) {
                    (None, _) => "Адрес не найден",
                    (Some(_), Some(err)) => err.text(),
                    (Some(_), None) => "Нет ответа",
                }
                .to_string()
            });
            points.push(PointResult {
                id: point.id.to_string(),
                label: point.label.to_string(),
                host: point.host.to_string(),
                address: found.address,
                ping_ms: latest.median_ms,
                min_ms: latest.min_ms,
                max_ms: latest.max_ms,
                jitter_ms: agg.jitter_ms,
                tail_ms: agg.tail_ms,
                loss: agg.loss,
                score: score(&agg),
                runs: history.len(),
                samples: history.iter().map(|r| r.total).sum(),
                error,
            });
        }
        let Advice { best, suggestion, preliminary } = pick(&points);
        let probe = Probe { points, best, suggestion, preliminary, checked_at: now };
        store.last = Some(probe.clone());
        save_store(&store);
        probe
    })
    .await
    .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn run(total: u32, failed: u32, median_ms: Option<f64>, jitter_ms: Option<f64>) -> RunSummary {
        RunSummary { at: 0, total, failed, median_ms, jitter_ms, min_ms: None, max_ms: None, tail_ms: None }
    }

    fn point(id: &str, score: Option<f64>) -> PointResult {
        PointResult {
            id: id.to_string(),
            label: id.to_string(),
            host: id.to_string(),
            address: String::new(),
            ping_ms: None,
            min_ms: None,
            max_ms: None,
            jitter_ms: None,
            tail_ms: None,
            loss: 0.0,
            score,
            runs: 3,
            samples: MIN_SAMPLES_FOR_ADVICE,
            error: None,
        }
    }

    fn few_answers(id: &str, score: Option<f64>) -> PointResult {
        PointResult { samples: MIN_SAMPLES_FOR_ADVICE - 1, ..point(id, score) }
    }

    fn scored(runs: &[RunSummary]) -> Option<f64> {
        score(&aggregate(runs))
    }

    fn no_tick() -> impl Fn() + Sync {
        || {}
    }

    #[test]
    fn a_summary_counts_the_answers_and_the_failures() {
        let s = summarize(5, &[Some(40.0), None, Some(50.0), Some(60.0)]);
        assert_eq!((s.total, s.failed), (4, 1));
        assert_eq!(s.median_ms, Some(50.0));
        assert_eq!((s.min_ms, s.max_ms), (Some(40.0), Some(60.0)));
        assert!((s.jitter_ms.unwrap() - 20.0 / 3.0).abs() < 1e-9);
        let dead = summarize(5, &[None, None, None]);
        assert_eq!((dead.total, dead.failed, dead.median_ms, dead.jitter_ms, dead.tail_ms), (3, 3, None, None, None));
    }

    #[test]
    fn the_median_of_an_even_count_is_the_middle_pair() {
        assert_eq!(median(&[10.0, 40.0, 20.0, 30.0]), Some(25.0));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn the_tail_is_the_ninetieth_percentile_by_nearest_rank() {
        assert_eq!(tail(&[10.0, 20.0, 30.0, 40.0, 50.0, 400.0]), Some(400.0));
        let many: Vec<f64> = (1..=24).map(f64::from).collect();
        assert_eq!(tail(&many), Some(22.0));
        assert_eq!(tail(&[7.0]), Some(7.0));
        assert_eq!(tail(&[]), None);
    }

    #[test]
    fn a_steady_slower_way_beats_a_fast_one_that_drops_requests() {
        let steady = scored(&[run(6, 0, Some(90.0), Some(2.0))]).unwrap();
        let lossy = scored(&[run(6, 1, Some(40.0), Some(3.0))]).unwrap();
        assert!(steady < lossy, "{steady} vs {lossy}");
    }

    #[test]
    fn jitter_counts_against_a_way() {
        let calm = scored(&[run(6, 0, Some(60.0), Some(1.0))]).unwrap();
        let shaky = scored(&[run(6, 0, Some(60.0), Some(30.0))]).unwrap();
        assert!(calm < shaky);
    }

    #[test]
    fn a_rare_spike_counts_against_a_way_with_the_same_middle() {
        let even = RunSummary { tail_ms: Some(70.0), ..run(6, 0, Some(60.0), Some(4.0)) };
        let spiky = RunSummary { tail_ms: Some(400.0), ..run(6, 0, Some(60.0), Some(4.0)) };
        assert!(scored(&[even]).unwrap() < scored(&[spiky]).unwrap());
    }

    #[test]
    fn checks_that_differ_from_each_other_raise_the_jitter() {
        let even = aggregate(&[run(6, 0, Some(50.0), Some(2.0)), run(6, 0, Some(50.0), Some(2.0))]);
        let uneven = aggregate(&[run(6, 0, Some(30.0), Some(2.0)), run(6, 0, Some(70.0), Some(2.0))]);
        assert!(uneven.jitter_ms.unwrap() > even.jitter_ms.unwrap());
        assert_eq!(even.jitter_ms, Some(2.0));
    }

    #[test]
    fn loss_adds_up_across_checks_and_a_dead_check_still_counts() {
        let agg = aggregate(&[run(6, 0, Some(40.0), Some(1.0)), run(3, 3, None, None)]);
        assert!((agg.loss - 3.0 / 9.0).abs() < 1e-9);
        assert_eq!(agg.median_ms, Some(40.0));
    }

    #[test]
    fn a_point_that_never_answered_has_no_score() {
        assert_eq!(scored(&[run(3, 3, None, None)]), None);
        assert_eq!(scored(&[]), None);
    }

    #[test]
    fn no_suggestion_when_the_direct_way_is_the_best() {
        let advice = pick(&[point("direct", Some(40.0)), point("de", Some(60.0))]);
        assert_eq!((advice.best.as_deref(), advice.suggestion, advice.preliminary), (Some("direct"), None, false));
    }

    #[test]
    fn a_slightly_better_relay_is_not_suggested() {
        // 30 % better but only 18 ms, and 20 % better by ratio: neither is enough
        let advice = pick(&[point("direct", Some(60.0)), point("de", Some(42.0))]);
        assert_eq!((advice.best.as_deref(), advice.suggestion, advice.preliminary), (Some("de"), None, false));
        assert_eq!(pick(&[point("direct", Some(400.0)), point("de", Some(350.0))]).suggestion, None);
    }

    #[test]
    fn a_clearly_better_relay_is_suggested() {
        let advice = pick(&[point("direct", Some(160.0)), point("de", Some(60.0)), point("nl", Some(90.0))]);
        assert_eq!((advice.best.as_deref(), advice.suggestion.as_deref(), advice.preliminary), (Some("de"), Some("de"), false));
    }

    #[test]
    fn a_clear_win_on_too_few_answers_is_only_preliminary() {
        let advice = pick(&[point("direct", Some(160.0)), few_answers("de", Some(60.0))]);
        assert_eq!((advice.best.as_deref(), advice.suggestion, advice.preliminary), (Some("de"), None, true));
        // the direct way being barely measured is as weak a basis
        let advice = pick(&[few_answers("direct", Some(160.0)), point("de", Some(60.0))]);
        assert_eq!((advice.suggestion, advice.preliminary), (None, true));
    }

    #[test]
    fn a_dead_direct_way_suggests_the_best_relay() {
        let advice = pick(&[few_answers("direct", None), point("nl", Some(120.0)), point("de", Some(200.0))]);
        assert_eq!(advice.suggestion.as_deref(), Some("nl"));
    }

    #[test]
    fn nothing_answering_suggests_nothing() {
        assert_eq!(pick(&[point("direct", None), point("de", None)]), Advice { best: None, suggestion: None, preliminary: false });
    }

    #[test]
    fn ways_close_to_the_best_or_uneven_are_asked_more_but_dead_and_far_ones_are_not() {
        let steady = |ms: f64| vec![Some(ms); 6];
        let rounds = vec![
            steady(90.0),                                                          // best
            steady(110.0),                                                         // close
            steady(300.0),                                                         // far and steady
            vec![Some(300.0), Some(300.0), None, Some(300.0), Some(300.0), Some(300.0)], // far but lost one
            vec![Some(300.0), Some(300.0), Some(300.0), Some(300.0), Some(300.0), Some(900.0)], // far but with a spike
            vec![None, None, None],                                                // dead
        ];
        assert_eq!(needs_more(&rounds), vec![true, true, false, true, true, false]);
        assert_eq!(needs_more(&[vec![None, None, None]]), vec![false]);
    }

    #[test]
    fn history_from_another_way_of_measuring_is_dropped() {
        let mut old = Store::default();
        old.runs.insert("de".to_string(), vec![run(6, 0, Some(480.0), Some(5.0))]);
        old.last = Some(Probe { points: vec![], best: None, suggestion: None, preliminary: false, checked_at: 1 });
        let fresh = current_or_fresh(old);
        assert!(fresh.runs.is_empty() && fresh.last.is_none());
        assert_eq!(fresh.metric, METRIC_VERSION);

        let mut current = Store { metric: METRIC_VERSION, ..Store::default() };
        current.runs.insert("de".to_string(), vec![run(6, 0, Some(120.0), Some(5.0))]);
        assert_eq!(current_or_fresh(current).runs.len(), 1);
    }

    #[test]
    fn old_and_surplus_checks_are_dropped() {
        let now = 10 * HISTORY_MAX_AGE_SECS;
        let mut runs: Vec<RunSummary> = (0..20).map(|i| RunSummary { at: now - i * 60, ..run(6, 0, Some(40.0), Some(1.0)) }).collect();
        runs.insert(0, RunSummary { at: now - HISTORY_MAX_AGE_SECS - 1, ..run(6, 6, None, None) });
        trim_history(&mut runs, now);
        assert_eq!(runs.len(), HISTORY_RUNS);
        assert!(runs.iter().all(|r| r.failed == 0));
    }

    #[test]
    fn a_name_answer_is_remembered_for_a_while_and_then_asked_again() {
        let mut cache = SrvCache::default();
        let now = Instant::now();
        assert_eq!(cache.get("de.lemicraft.ru", now), None);
        cache.put("de.lemicraft.ru", Some(("de.lemicraft.ru".to_string(), 25565)), now);
        cache.put("nl.lemicraft.ru", None, now);
        assert_eq!(cache.get("de.lemicraft.ru", now + Duration::from_secs(5)), Some(Some(("de.lemicraft.ru".to_string(), 25565))));
        assert_eq!(cache.get("nl.lemicraft.ru", now + Duration::from_secs(5)), Some(None), "no SRV record is an answer too");
        assert_eq!(cache.get("de.lemicraft.ru", now + SRV_TTL + Duration::from_secs(1)), None);
    }

    #[test]
    fn connection_errors_get_their_own_reason() {
        use std::io::{Error, ErrorKind};
        assert_eq!(classify(&Error::from(ErrorKind::ConnectionRefused)), ProbeError::Refused);
        assert_eq!(classify(&Error::from(ErrorKind::ConnectionReset)), ProbeError::Reset);
        assert_eq!(classify(&Error::from(ErrorKind::UnexpectedEof)), ProbeError::Reset);
        assert_eq!(classify(&Error::from(ErrorKind::TimedOut)), ProbeError::Timeout);
        assert_eq!(classify(&Error::from(ErrorKind::PermissionDenied)), ProbeError::Other);
        assert!(!ProbeError::Refused.text().is_empty());
    }

    #[test]
    fn an_ipv4_address_is_taken_before_an_ipv6_one() {
        let v6 = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 25565));
        let v4 = SocketAddr::from(([10, 0, 0, 1], 25565));
        assert_eq!(prefer_ipv4([v6, v4]), Some(v4));
        assert_eq!(prefer_ipv4([v6]), Some(v6));
        assert_eq!(prefer_ipv4([]), None);
    }

    #[test]
    fn the_auto_connect_goes_where_the_player_chose_or_where_the_last_check_recommended() {
        let probe = |suggestion: Option<&str>, at: u64| Probe { points: vec![], best: None, suggestion: suggestion.map(str::to_string), preliminary: false, checked_at: at };
        let now = 10 * AUTO_ROUTE_MAX_AGE_SECS;
        assert_eq!(host_for_route("direct", None, now), "lemicraft.ru");
        assert_eq!(host_for_route("de", None, now), "de.lemicraft.ru");
        assert_eq!(host_for_route("nl", Some(&probe(Some("ru"), now)), now), "nl.lemicraft.ru", "an explicit choice is not overridden");
        assert_eq!(host_for_route("auto", Some(&probe(Some("ru"), now - 60)), now), "ru.lemicraft.ru");
        assert_eq!(host_for_route("auto", Some(&probe(None, now)), now), "lemicraft.ru", "nothing recommended means the direct way");
        assert_eq!(host_for_route("auto", Some(&probe(Some("ru"), now - AUTO_ROUTE_MAX_AGE_SECS - 1)), now), "lemicraft.ru", "an old recommendation is not used");
        assert_eq!(host_for_route("auto", None, now), "lemicraft.ru");
        assert_eq!(host_for_route("nowhere", None, now), "lemicraft.ru");
    }

    #[test]
    fn the_mode_name_picks_the_size_of_the_check() {
        assert_eq!(Mode::parse(Some("deep")), Mode::Deep);
        assert_eq!(Mode::parse(Some("quick")), Mode::Quick);
        assert_eq!(Mode::parse(None), Mode::Quick);
        assert!(Mode::Deep.samples() > Mode::Quick.samples());
    }

    #[derive(Clone, Copy)]
    enum Reply {
        Full,
        StatusOnly,
        HangUp,
    }

    // Local stand-in for a game server: answers a status request and the ping after it, only the status, or hangs up
    fn status_server(replies: Vec<Reply>) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for reply in replies {
                let Ok((mut stream, _)) = listener.accept() else { return };
                if matches!(reply, Reply::HangUp) {
                    continue;
                }
                // take the whole request first: closing with unread data would reset the connection
                let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
                let mut buf = [0u8; 256];
                while stream.read(&mut buf).is_ok_and(|n| n > 0) {}
                let json = br#"{"version":{"name":"x"},"players":{"online":1,"max":2},"description":"hi"}"#;
                let mut body = vec![0x00];
                crate::server_status::write_varint(&mut body, json.len() as i32);
                body.extend_from_slice(json);
                let mut packet = Vec::new();
                crate::server_status::write_varint(&mut packet, body.len() as i32);
                packet.extend(body);
                let _ = stream.write_all(&packet);
                if matches!(reply, Reply::Full) {
                    // the client pings after the status reply: echo it back like a game server does
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                    if let Ok(n) = stream.read(&mut buf) {
                        let _ = stream.write_all(&buf[..n]);
                    }
                } else {
                    // hold the connection open without echoing, so the client has to give up
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        });
        port
    }

    fn local(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
    }

    const TEST_GAP: Duration = Duration::from_millis(10);

    #[test]
    fn a_real_exchange_is_measured_and_a_dropped_one_counts_as_lost() {
        let port = status_server(vec![Reply::Full, Reply::HangUp, Reply::Full, Reply::Full]);
        let (rtts, error) = block_on(sample(local(port), "lemicraft.ru", 4, TEST_GAP, &no_tick()));
        assert_eq!(rtts.len(), 4);
        assert_eq!(rtts.iter().map(Option::is_some).collect::<Vec<_>>(), vec![true, false, true, true]);
        assert!(rtts.iter().flatten().all(|ms| *ms >= 0.0 && *ms < 2500.0));
        assert_eq!(error, Some(ProbeError::Reset), "the hang-up leaves its reason");
    }

    #[test]
    fn a_status_without_a_ping_echo_counts_as_lost() {
        let port = status_server(vec![Reply::StatusOnly]);
        let (rtts, error) = block_on(sample(local(port), "lemicraft.ru", 1, TEST_GAP, &no_tick()));
        assert_eq!(rtts, vec![None]);
        assert!(matches!(error, Some(ProbeError::NoPing | ProbeError::Reset | ProbeError::Timeout)), "{error:?}");
    }

    #[test]
    fn every_request_reports_progress() {
        let port = status_server(vec![Reply::Full, Reply::Full, Reply::Full]);
        let ticks = AtomicUsize::new(0);
        let tick = || {
            ticks.fetch_add(1, Ordering::Relaxed);
        };
        block_on(sample(local(port), "lemicraft.ru", 3, TEST_GAP, &tick));
        assert_eq!(ticks.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn a_point_that_never_answers_is_written_off_after_a_few_tries() {
        let port = status_server(vec![Reply::HangUp; 6]);
        let (rtts, error) = block_on(sample(local(port), "lemicraft.ru", 6, TEST_GAP, &no_tick()));
        assert_eq!(rtts, vec![None; GIVE_UP_AFTER]);
        assert!(error.is_some());
    }

    #[test]
    fn a_closed_port_fails_fast_and_says_why() {
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let start = Instant::now();
        let (rtts, error) = block_on(sample(local(free), "lemicraft.ru", 6, TEST_GAP, &no_tick()));
        assert_eq!(rtts, vec![None; GIVE_UP_AFTER]);
        // a refused connection can take a couple of seconds to be reported on Windows
        assert!(start.elapsed() < (SAMPLE_TIMEOUT + TEST_GAP) * GIVE_UP_AFTER as u32);
        assert!(matches!(error, Some(ProbeError::Refused | ProbeError::Timeout)), "{error:?}");
    }

    // Against the real addresses; run by hand: cargo test --lib real_addresses -- --ignored --nocapture
    #[test]
    #[ignore = "needs network"]
    fn real_addresses_answer_and_get_a_score() {
        let results = block_on(async {
            let handles: Vec<_> = POINTS.iter().map(|p| tokio::spawn(async move { probe_point(p, QUICK_SAMPLES, QUICK_GAP, &|| {}).await })).collect();
            let mut out = Vec::new();
            for h in handles {
                out.push(h.await.unwrap());
            }
            out
        });
        let mut points = Vec::new();
        for (point, found) in POINTS.iter().zip(results) {
            let run = summarize(0, &found.rtts);
            let agg = aggregate(std::slice::from_ref(&run));
            println!(
                "{:<16} {:<28} ok {}/{} ping {:?} range {:?}-{:?} tail {:?} jitter {:?} score {:?} err {:?}",
                point.label,
                found.address,
                run.total - run.failed,
                run.total,
                run.median_ms.map(f64::round),
                run.min_ms.map(f64::round),
                run.max_ms.map(f64::round),
                run.tail_ms.map(f64::round),
                agg.jitter_ms.map(|v| (v * 10.0).round() / 10.0),
                score(&agg).map(f64::round),
                found.error.map(ProbeError::text),
            );
            points.push(PointResult { address: found.address, ..point_with(point.id, score(&agg)) });
        }
        println!("advice: {:?}", pick(&points));
    }

    fn point_with(id: &str, score: Option<f64>) -> PointResult {
        point(id, score)
    }
}
