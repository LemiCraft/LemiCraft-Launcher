// Prototype: measures every way into the server (direct and through the relays) and says which is
// best. It only advises; the address the game uses is not touched here.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::time::timeout;

use crate::server_status::{lookup_srv, ping_target};

const SAMPLES: usize = 6;
const SAMPLE_GAP: Duration = Duration::from_millis(300);
const SAMPLE_TIMEOUT: Duration = Duration::from_millis(2500);
const SRV_TIMEOUT: Duration = Duration::from_secs(4);
// a point that fails this many times from the start is written off, so a dead relay does not hold the check up
const GIVE_UP_AFTER: usize = 3;
const DEFAULT_PORT: u16 = 25565;

const HISTORY_RUNS: usize = 12;
const HISTORY_MAX_AGE_SECS: u64 = 24 * 60 * 60;

// every 10 % of lost requests costs as much as this many ms of delay
const LOSS_COST_MS: f64 = 1000.0;
const JITTER_WEIGHT: f64 = 2.0;
// the direct way stays the advice unless another is clearly better
const BETTER_BY_RATIO: f64 = 0.75;
const BETTER_BY_MS: f64 = 20.0;

const DIRECT_ID: &str = "direct";

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

/// What one check of one point found: how many requests went out, how many failed, and their spread
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RunSummary {
    pub at: u64,
    pub total: u32,
    pub failed: u32,
    pub median_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PointResult {
    pub id: String,
    pub label: String,
    pub host: String,
    pub address: String,
    /// Delay of the latest check
    pub ping_ms: Option<f64>,
    /// Over the last day of checks
    pub jitter_ms: Option<f64>,
    pub loss: f64,
    /// Lower is better; None when nothing ever answered
    pub score: Option<f64>,
    pub runs: usize,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Probe {
    pub points: Vec<PointResult>,
    /// Point with the best score
    pub best: Option<String>,
    /// Point worth switching to, empty when the direct way is good enough
    pub suggestion: Option<String>,
    pub checked_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    runs: HashMap<String, Vec<RunSummary>>,
    last: Option<Probe>,
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 { sorted[mid] } else { (sorted[mid - 1] + sorted[mid]) / 2.0 })
}

// mean distance from the middle value; a few samples are too few for a proper deviation
fn spread(values: &[f64]) -> Option<f64> {
    let mid = median(values)?;
    Some(values.iter().map(|v| (v - mid).abs()).sum::<f64>() / values.len() as f64)
}

pub fn summarize(at: u64, rtts: &[Option<f64>]) -> RunSummary {
    let answered: Vec<f64> = rtts.iter().flatten().copied().collect();
    RunSummary {
        at,
        total: rtts.len() as u32,
        failed: (rtts.len() - answered.len()) as u32,
        median_ms: median(&answered),
        jitter_ms: spread(&answered),
    }
}

#[derive(Debug, PartialEq)]
pub struct Aggregate {
    pub loss: f64,
    pub median_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
}

/// Several checks in a row show what one cannot: a point that is fast but drops out now and then
pub fn aggregate(history: &[RunSummary]) -> Aggregate {
    let total: u32 = history.iter().map(|r| r.total).sum();
    let failed: u32 = history.iter().map(|r| r.failed).sum();
    let medians: Vec<f64> = history.iter().filter_map(|r| r.median_ms).collect();
    let jitters: Vec<f64> = history.iter().filter_map(|r| r.jitter_ms).collect();
    // the jitter inside each check plus how much the checks differ from each other
    let jitter_ms = match (jitters.is_empty(), spread(&medians)) {
        (false, Some(between)) => Some(jitters.iter().sum::<f64>() / jitters.len() as f64 + between),
        _ => None,
    };
    Aggregate {
        loss: if total == 0 { 0.0 } else { failed as f64 / total as f64 },
        median_ms: median(&medians),
        jitter_ms,
    }
}

pub fn score(agg: &Aggregate) -> Option<f64> {
    let delay = agg.median_ms?;
    Some(delay + JITTER_WEIGHT * agg.jitter_ms.unwrap_or(0.0) + LOSS_COST_MS * agg.loss)
}

/// (best point, point to suggest). The suggestion needs a clear win over the direct way.
pub fn pick(points: &[PointResult]) -> (Option<String>, Option<String>) {
    let best = points
        .iter()
        .filter_map(|p| p.score.map(|s| (p, s)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((best, best_score)) = best else { return (None, None) };
    if best.id == DIRECT_ID {
        return (Some(best.id.clone()), None);
    }
    let direct = points.iter().find(|p| p.id == DIRECT_ID).and_then(|p| p.score);
    let clearly_better = match direct {
        None => true,
        Some(d) => best_score <= d * BETTER_BY_RATIO && d - best_score >= BETTER_BY_MS,
    };
    (Some(best.id.clone()), clearly_better.then(|| best.id.clone()))
}

async fn resolve(point: &Point) -> (String, u16) {
    match timeout(SRV_TIMEOUT, lookup_srv(point.host)).await {
        Ok(Ok(target)) => target,
        _ => (point.fallback.0.to_string(), point.fallback.1),
    }
}

/// Delays of the requests that came back, None for those that did not
async fn sample(connect_host: &str, port: u16, handshake_host: &str, samples: usize) -> (Vec<Option<f64>>, Option<String>) {
    let mut rtts = Vec::with_capacity(samples);
    let mut last_error = None;
    for i in 0..samples {
        if i > 0 {
            tokio::time::sleep(SAMPLE_GAP).await;
        }
        let start = Instant::now();
        match timeout(SAMPLE_TIMEOUT, ping_target(connect_host, port, handshake_host)).await {
            Ok(Ok(_)) => rtts.push(Some(start.elapsed().as_secs_f64() * 1000.0)),
            Ok(Err(err)) => {
                rtts.push(None);
                last_error = Some(err);
            }
            Err(_) => {
                rtts.push(None);
                last_error = Some("Тайм-аут подключения".to_string());
            }
        }
        if rtts.len() == GIVE_UP_AFTER && rtts.iter().all(Option::is_none) {
            break;
        }
    }
    (rtts, last_error)
}

async fn probe_point(point: &'static Point) -> (String, Vec<Option<f64>>, Option<String>) {
    let (host, port) = resolve(point).await;
    let (rtts, error) = sample(&host, port, point.host, SAMPLES).await;
    (format!("{host}:{port}"), rtts, error)
}

fn store_path() -> std::path::PathBuf {
    crate::game::game_dir().join("entry_points.json")
}

fn load_store() -> Store {
    std::fs::read_to_string(store_path())
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
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

#[tauri::command]
pub async fn probe_entry_points() -> Result<Probe, String> {
    let handles: Vec<_> = POINTS.iter().map(|p| tauri::async_runtime::spawn(probe_point(p))).collect();
    let mut sampled = Vec::with_capacity(handles.len());
    for handle in handles {
        sampled.push(handle.await.map_err(|err| err.to_string())?);
    }

    tauri::async_runtime::spawn_blocking(move || {
        let now = crate::auth::now_unix();
        let mut store = load_store();
        let mut points = Vec::with_capacity(POINTS.len());
        for (point, (address, rtts, error)) in POINTS.iter().zip(sampled) {
            let latest = summarize(now, &rtts);
            let history = store.runs.entry(point.id.to_string()).or_default();
            history.push(latest.clone());
            trim_history(history, now);
            let agg = aggregate(history);
            points.push(PointResult {
                id: point.id.to_string(),
                label: point.label.to_string(),
                host: point.host.to_string(),
                address,
                ping_ms: latest.median_ms,
                jitter_ms: agg.jitter_ms,
                loss: agg.loss,
                score: score(&agg),
                runs: history.len(),
                error: if latest.median_ms.is_none() { error } else { None },
            });
        }
        let (best, suggestion) = pick(&points);
        let probe = Probe { points, best, suggestion, checked_at: now };
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
        RunSummary { at: 0, total, failed, median_ms, jitter_ms }
    }

    fn point(id: &str, score: Option<f64>) -> PointResult {
        PointResult {
            id: id.to_string(),
            label: id.to_string(),
            host: id.to_string(),
            address: String::new(),
            ping_ms: None,
            jitter_ms: None,
            loss: 0.0,
            score,
            runs: 1,
            error: None,
        }
    }

    fn point_result(id: &str, score: Option<f64>) -> PointResult {
        point(id, score)
    }

    fn scored(runs: &[RunSummary]) -> Option<f64> {
        score(&aggregate(runs))
    }

    #[test]
    fn a_summary_counts_the_answers_and_the_failures() {
        let s = summarize(5, &[Some(40.0), None, Some(50.0), Some(60.0)]);
        assert_eq!((s.total, s.failed), (4, 1));
        assert_eq!(s.median_ms, Some(50.0));
        assert!((s.jitter_ms.unwrap() - 20.0 / 3.0).abs() < 1e-9);
        let dead = summarize(5, &[None, None, None]);
        assert_eq!((dead.total, dead.failed, dead.median_ms, dead.jitter_ms), (3, 3, None, None));
    }

    #[test]
    fn the_median_of_an_even_count_is_the_middle_pair() {
        assert_eq!(median(&[10.0, 40.0, 20.0, 30.0]), Some(25.0));
        assert_eq!(median(&[]), None);
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
        let (best, suggestion) = pick(&[point("direct", Some(40.0)), point("de", Some(60.0))]);
        assert_eq!((best.as_deref(), suggestion), (Some("direct"), None));
    }

    #[test]
    fn a_slightly_better_relay_is_not_suggested() {
        // 30 % better but only 18 ms, and 20 % better by ratio: neither is enough
        let (best, suggestion) = pick(&[point("direct", Some(60.0)), point("de", Some(42.0))]);
        assert_eq!((best.as_deref(), suggestion), (Some("de"), None));
        let (_, suggestion) = pick(&[point("direct", Some(400.0)), point("de", Some(350.0))]);
        assert_eq!(suggestion, None);
    }

    #[test]
    fn a_clearly_better_relay_is_suggested() {
        let (best, suggestion) = pick(&[point("direct", Some(160.0)), point("de", Some(60.0)), point("nl", Some(90.0))]);
        assert_eq!((best.as_deref(), suggestion.as_deref()), (Some("de"), Some("de")));
    }

    #[test]
    fn a_dead_direct_way_suggests_the_best_relay() {
        let (_, suggestion) = pick(&[point("direct", None), point("nl", Some(120.0)), point("de", Some(200.0))]);
        assert_eq!(suggestion.as_deref(), Some("nl"));
    }

    #[test]
    fn nothing_answering_suggests_nothing() {
        assert_eq!(pick(&[point("direct", None), point("de", None)]), (None, None));
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

    // Local stand-in for a game server: answers a status request with a valid reply, or just hangs up
    fn status_server(answers: Vec<bool>) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for answer in answers {
                let Ok((mut stream, _)) = listener.accept() else { return };
                if !answer {
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
            }
        });
        port
    }

    fn block_on_resolve(point: &Point) -> (String, u16) {
        block_on(resolve(point))
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
    }

    #[test]
    fn a_real_status_exchange_is_measured_and_a_dropped_one_counts_as_lost() {
        let port = status_server(vec![true, false, true, true]);
        let (rtts, error) = block_on(sample("127.0.0.1", port, "lemicraft.ru", 4));
        assert_eq!(rtts.len(), 4);
        assert_eq!(rtts.iter().map(Option::is_some).collect::<Vec<_>>(), vec![true, false, true, true]);
        assert!(rtts.iter().flatten().all(|ms| *ms >= 0.0 && *ms < 2500.0));
        assert!(error.is_some(), "the failed request leaves its reason");
    }

    #[test]
    fn a_point_that_never_answers_is_written_off_after_a_few_tries() {
        let port = status_server(vec![false; 6]);
        let (rtts, error) = block_on(sample("127.0.0.1", port, "lemicraft.ru", 6));
        assert_eq!(rtts, vec![None; GIVE_UP_AFTER]);
        assert!(error.is_some());
    }

    // Against the real addresses; run by hand: cargo test --lib real_addresses -- --ignored --nocapture
    #[test]
    #[ignore = "needs network"]
    fn real_addresses_answer_and_get_a_score() {
        let results = block_on(async {
            let handles: Vec<_> = POINTS.iter().map(|p| tokio::spawn(probe_point(p))).collect();
            let mut out = Vec::new();
            for h in handles {
                out.push(h.await.unwrap());
            }
            out
        });
        let mut points = Vec::new();
        for (point, (address, rtts, error)) in POINTS.iter().zip(results) {
            let t = Instant::now();
            let resolved = block_on_resolve(point);
            println!("   resolve {:?} in {:?}", resolved, t.elapsed());
            let run = summarize(0, &rtts);
            let agg = aggregate(std::slice::from_ref(&run));
            println!("{:<16} {:<28} ok {}/{} ping {:?} jitter {:?} score {:?} err {:?}", point.label, address, run.total - run.failed, run.total, run.median_ms.map(|v| v.round()), agg.jitter_ms.map(|v| (v * 10.0).round() / 10.0), score(&agg).map(|v| v.round()), error);
            let mut p = point_result(point.id, score(&agg));
            p.address = address;
            points.push(p);
        }
        println!("pick: {:?}", pick(&points));
    }

    #[test]
    fn a_closed_port_fails_fast() {
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let start = Instant::now();
        let (rtts, _) = block_on(sample("127.0.0.1", free, "lemicraft.ru", 6));
        assert_eq!(rtts, vec![None; GIVE_UP_AFTER]);
        // a refused connection can take a couple of seconds to be reported on Windows
        assert!(start.elapsed() < (SAMPLE_TIMEOUT + SAMPLE_GAP) * GIVE_UP_AFTER as u32);
    }
}
