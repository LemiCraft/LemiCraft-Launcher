use std::io::Cursor;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: u32 = 3;

// Resolved once per process, not on every 120s poll
static RESOLVED_TARGET: OnceLock<Mutex<Option<(String, u16)>>> = OnceLock::new();

async fn resolve_srv() -> (String, u16) {
    let lookup = async {
        let resolver = hickory_resolver::Resolver::builder_tokio()
            .map_err(|err| err.to_string())?
            .build()
            .map_err(|err| err.to_string())?;
        let response = resolver
            .srv_lookup(format!("_minecraft._tcp.{}.", crate::config::SERVER_SRV_DOMAIN))
            .await
            .map_err(|err| err.to_string())?;
        for record in response.answers() {
            if let hickory_resolver::proto::rr::RData::SRV(srv) = &record.data {
                return Ok((srv.target.to_string().trim_end_matches('.').to_string(), srv.port));
            }
        }
        Err("SRV-запись не найдена".to_string())
    };

    match lookup.await {
        Ok(target) => target,
        Err(_) => (crate::config::SERVER_HOST.to_string(), crate::config::SERVER_PORT),
    }
}

async fn resolve_target() -> (String, u16) {
    let cell = RESOLVED_TARGET.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().await;
    if let Some(target) = guard.as_ref() {
        return target.clone();
    }
    let target = resolve_srv().await;
    *guard = Some(target.clone());
    target
}

// Anti-bot layer drops pings with any protocol version other than 754
const PROTOCOL_VERSION: i32 = 754;

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerStatusInfo {
    online: bool,
    players_online: i64,
    players_max: i64,
    version: String,
    latency_ms: f64,
    motd: String,
}

fn status_cache_path() -> std::path::PathBuf {
    crate::game::game_dir().join("server_status_cache.json")
}

const STATUS_FALLBACK_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Instant first paint from disk, no network call — the live ping still runs right after
#[tauri::command]
pub fn get_cached_server_status() -> Option<ServerStatusInfo> {
    crate::skin::read_cache_if_fresh(&status_cache_path(), STATUS_FALLBACK_MAX_AGE)
        .and_then(|json| serde_json::from_str(&json).ok())
}

#[tauri::command]
pub async fn get_server_status() -> Result<ServerStatusInfo, String> {
    match get_server_status_live().await {
        Ok(info) => {
            let cached = info.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if let Ok(json) = serde_json::to_string(&cached) {
                    let _ = std::fs::write(status_cache_path(), json);
                }
            })
            .await;
            Ok(info)
        }
        Err(err) => tauri::async_runtime::spawn_blocking(|| {
            crate::skin::read_cache_if_fresh(&status_cache_path(), STATUS_FALLBACK_MAX_AGE)
                .and_then(|json| serde_json::from_str(&json).ok())
        })
        .await
        .ok()
        .flatten()
        .ok_or(err),
    }
}

async fn get_server_status_live() -> Result<ServerStatusInfo, String> {
    let mut last_err = String::new();
    let mut result = None;
    for attempt in 1..=MAX_ATTEMPTS {
        let start = Instant::now();
        match timeout(TIMEOUT, ping()).await {
            Ok(Ok(value)) => {
                result = Some((value, start.elapsed().as_secs_f64() * 1000.0));
                break;
            }
            Ok(Err(err)) => last_err = err,
            Err(_) => last_err = "Тайм-аут подключения к серверу".to_string(),
        }
        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    let (result, latency_ms) = result.ok_or(last_err)?;

    let players = result.get("players");
    let players_online = players.and_then(|p| p.get("online")).and_then(Value::as_i64).unwrap_or(0);
    let players_max = players.and_then(|p| p.get("max")).and_then(Value::as_i64).unwrap_or(0);
    let version = result
        .get("version")
        .and_then(|v| v.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let motd = result.get("description").map(motd_text).unwrap_or_default();

    Ok(ServerStatusInfo { online: true, players_online, players_max, version, latency_ms, motd })
}

/// Flattens a chat-component `description` field (plain string, or
/// `{text, extra:[...]}`) into plain text, stripping `§`-color codes.
fn motd_text(desc: &Value) -> String {
    fn collect(v: &Value, out: &mut String) {
        match v {
            Value::String(s) => out.push_str(s),
            Value::Object(_) => {
                if let Some(Value::String(s)) = v.get("text") {
                    out.push_str(s);
                }
                if let Some(Value::Array(extra)) = v.get("extra") {
                    for item in extra {
                        collect(item, out);
                    }
                }
            }
            _ => {}
        }
    }
    let mut raw = String::new();
    collect(desc, &mut raw);

    let mut clean = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\u{00A7}' {
            chars.next();
        } else {
            clean.push(c);
        }
    }
    clean
}

async fn ping() -> Result<Value, String> {
    let (host, port) = resolve_target().await;
    let mut stream = TcpStream::connect((host.as_str(), port)).await.map_err(|err| err.to_string())?;
    stream.set_nodelay(true).map_err(|err| err.to_string())?;

    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, PROTOCOL_VERSION);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);
    write_packet(&mut stream, &handshake).await?;
    write_packet(&mut stream, &[0x00]).await?;

    let mut resp = Vec::new();
    let mut buf = [0u8; 4096];
    let mut expected = None;
    loop {
        let n = stream.read(&mut buf).await.map_err(|err| err.to_string())?;
        if n == 0 {
            break;
        }
        resp.extend_from_slice(&buf[..n]);
        if expected.is_none() && resp.len() >= 5 {
            let mut cur = Cursor::new(&resp);
            if let Ok(len) = read_varint(&mut cur) {
                expected = Some(cur.position() as usize + len as usize);
            }
        }
        if expected.is_some_and(|e| resp.len() >= e) {
            break;
        }
    }
    if resp.is_empty() {
        return Err("Пустой ответ от сервера".to_string());
    }

    let mut cur = Cursor::new(&resp);
    let _packet_len = read_varint(&mut cur)?;
    let packet_id = read_varint(&mut cur)?;
    if packet_id != 0x00 {
        return Err(format!("Неожиданный id пакета: {packet_id}"));
    }
    let json_len = read_varint(&mut cur)? as usize;
    let pos = cur.position() as usize;
    let end = pos.checked_add(json_len).filter(|&e| e <= resp.len()).ok_or("Некорректная длина JSON в ответе")?;

    serde_json::from_slice(&resp[pos..end]).map_err(|err| err.to_string())
}

async fn write_packet(stream: &mut TcpStream, data: &[u8]) -> Result<(), String> {
    let mut packet = Vec::with_capacity(data.len() + 5);
    write_varint(&mut packet, data.len() as i32);
    packet.extend_from_slice(data);
    stream.write_all(&packet).await.map_err(|err| err.to_string())
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            buf.push(v as u8);
            return;
        }
        buf.push((v as u8 & 0x7F) | 0x80);
        v >>= 7;
    }
}

fn read_varint(cur: &mut Cursor<&Vec<u8>>) -> Result<i32, String> {
    let (mut result, mut shift) = (0i32, 0u32);
    loop {
        let mut b = [0u8; 1];
        std::io::Read::read_exact(cur, &mut b).map_err(|err| err.to_string())?;
        result |= ((b[0] & 0x7F) as i32) << shift;
        shift += 7;
        if shift > 35 {
            return Err("varint слишком длинный".to_string());
        }
        if b[0] & 0x80 == 0 {
            break;
        }
    }
    Ok(result)
}
