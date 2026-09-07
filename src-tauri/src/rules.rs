use serde::{Deserialize, Serialize};

fn rules_api_url() -> String {
    format!("{}/launcher/rules", crate::config::API_BASE)
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RuleItem {
    id: String,
    text: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RuleNote {
    kind: String,
    text: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RuleSection {
    number: u32,
    emoji: String,
    title: String,
    rules: Vec<RuleItem>,
    #[serde(default)]
    notes: Vec<RuleNote>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RulesDoc {
    #[serde(rename = "updatedAt")]
    updated_at: String,
    sections: Vec<RuleSection>,
    footer: Vec<String>,
}

const RULES_FALLBACK_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);

fn rules_cache_path() -> std::path::PathBuf {
    crate::game::game_dir().join("rules_cache.json")
}

#[tauri::command]
pub fn get_cached_rules() -> Option<RulesDoc> {
    std::fs::read_to_string(rules_cache_path()).ok().and_then(|json| serde_json::from_str(&json).ok())
}

#[tauri::command]
pub async fn get_rules() -> Result<RulesDoc, String> {
    match get_rules_live().await {
        Ok(doc) => {
            let cached = doc.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if let Ok(json) = serde_json::to_string(&cached) {
                    let _ = std::fs::write(rules_cache_path(), json);
                }
            })
            .await;
            Ok(doc)
        }
        Err(err) => tauri::async_runtime::spawn_blocking(|| {
            crate::skin::read_cache_if_fresh(&rules_cache_path(), RULES_FALLBACK_MAX_AGE)
                .and_then(|json| serde_json::from_str(&json).ok())
        })
        .await
        .ok()
        .flatten()
        .ok_or(err),
    }
}

async fn get_rules_live() -> Result<RulesDoc, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;
    let res = client.get(rules_api_url()).send().await.map_err(|err| err.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Rules API returned {}", res.status()));
    }
    res.json::<RulesDoc>().await.map_err(|err| err.to_string())
}
