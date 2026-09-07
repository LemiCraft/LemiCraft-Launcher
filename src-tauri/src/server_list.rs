use serde::{Deserialize, Serialize};

use crate::game::minecraft_dir;

#[derive(Serialize, Deserialize, Clone)]
struct ServerEntry {
    ip: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(rename = "acceptTextures", skip_serializing_if = "Option::is_none")]
    accept_textures: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hidden: Option<i8>,
    #[serde(rename = "acceptedCodeOfConduct", skip_serializing_if = "Option::is_none")]
    accepted_code_of_conduct: Option<i32>,
}

#[derive(Serialize, Deserialize, Default)]
struct ServersDat {
    #[serde(default)]
    servers: Vec<ServerEntry>,
}

fn servers_dat_path() -> std::path::PathBuf {
    minecraft_dir().join("servers.dat")
}

// Bare domain — Minecraft resolves the actual proxy host/port itself via the SRV record,
// same as server_status.rs's own ping logic
fn lemicraft_ip() -> String {
    crate::config::SERVER_SRV_DOMAIN.to_string()
}

enum LoadResult {
    /// No file yet — safe to start from an empty list
    Empty,
    Parsed(ServersDat),
    /// File exists but couldn't be read as this NBT shape — must NOT be treated as empty,
    /// or the next write below would silently replace it with just our own entry
    ParseFailed,
}

// servers.dat is uncompressed NBT (unlike level.dat) — a bare compound with a `servers` list
fn load_servers_dat() -> LoadResult {
    match std::fs::read(servers_dat_path()) {
        Ok(bytes) => match fastnbt::from_bytes::<ServersDat>(&bytes) {
            Ok(doc) => LoadResult::Parsed(doc),
            Err(err) => {
                log::warn!("servers.dat exists but failed to parse, leaving it untouched: {err}");
                LoadResult::ParseFailed
            }
        },
        Err(_) => LoadResult::Empty,
    }
}

// Called from run() on every launch (install() itself is idempotent and runs every time, not just
// on a fresh install) — so this just keeps re-adding the entry if it's ever missing, on purpose
pub fn ensure_lemicraft_server_entry() {
    let mut doc = match load_servers_dat() {
        LoadResult::Parsed(doc) => doc,
        LoadResult::Empty => ServersDat::default(),
        LoadResult::ParseFailed => return,
    };
    let ip = lemicraft_ip();

    match doc.servers.iter_mut().find(|s| s.ip == ip) {
        Some(entry) => {
            // Two independent things an old/foreign entry could have wrong — fix whichever applies,
            // each checked on its own so a launch after only one was fixed still catches the other
            let mut changed = false;
            if entry.hidden == Some(1) {
                entry.hidden = None;
                changed = true;
            }
            if entry.name != "LemiCraft" {
                entry.name = "LemiCraft".to_string();
                changed = true;
            }
            if !changed {
                return;
            }
        }
        None => {
            doc.servers.insert(
                0,
                ServerEntry { ip, name: "LemiCraft".to_string(), icon: None, accept_textures: Some(1), hidden: None, accepted_code_of_conduct: None },
            );
        }
    }

    if let Ok(bytes) = fastnbt::to_bytes(&doc) {
        let _ = std::fs::create_dir_all(minecraft_dir());
        if let Err(err) = std::fs::write(servers_dat_path(), bytes) {
            log::warn!("failed to write servers.dat: {err}");
        }
    }
}
