use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::game::game_dir;

#[derive(Serialize, Deserialize, Clone)]
pub struct LauncherSettings {
    #[serde(default = "default_ram_gb")]
    pub ram_gb: u32,
    #[serde(default = "default_jvm_args")]
    pub jvm_args: String,
    #[serde(default)]
    pub auto_connect: bool,
    // "none" | "hide" | "close" — what happens to the window once the game process starts
    #[serde(default = "default_on_launch")]
    pub on_launch: String,
    #[serde(default)]
    pub game_dir: Option<String>,
    #[serde(default)]
    pub show_logs: bool,
    #[serde(default = "default_true")]
    pub crash_analyzer: bool,
}

fn default_ram_gb() -> u32 {
    4
}

fn default_true() -> bool {
    true
}

fn default_jvm_args() -> String {
    "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions".to_string()
}

fn default_on_launch() -> String {
    "hide".to_string()
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            ram_gb: default_ram_gb(),
            jvm_args: default_jvm_args(),
            auto_connect: false,
            on_launch: default_on_launch(),
            game_dir: None,
            show_logs: false,
            crash_analyzer: true,
        }
    }
}

#[tauri::command]
pub fn get_total_ram_gb() -> u32 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_gb = sys.total_memory() / (1024 * 1024 * 1024);
    if total_gb == 0 {
        16
    } else {
        total_gb as u32
    }
}

fn settings_path() -> PathBuf {
    game_dir().join("launcher_settings.json")
}

pub fn load() -> LauncherSettings {
    fs::read_to_string(settings_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_settings() -> LauncherSettings {
    load()
}

#[tauri::command]
pub fn save_settings(settings: LauncherSettings) -> Result<(), String> {
    fs::create_dir_all(game_dir()).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    fs::write(settings_path(), json).map_err(|err| err.to_string())
}
