use serde::{Deserialize, Serialize};

fn violations_api_url() -> String {
    format!("{}/launcher/violations", crate::config::API_BASE)
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Violation {
    #[serde(rename = "type")]
    kind: String,
    reason: String,
    #[serde(rename = "issuedAt")]
    issued_at: i64,
    #[serde(rename = "endsAt")]
    ends_at: Option<i64>,
    permanent: bool,
    #[serde(rename = "issuedBy")]
    issued_by: Option<String>,
}

#[derive(Deserialize)]
struct ViolationsResponse {
    violations: Vec<Violation>,
}

// Live-only, no disk cache — a stale mute/ban status is actively misleading, unlike news/mods,
// and caching by username on shared disk would leak one account's record to the next login
#[tauri::command]
pub async fn get_violations(username: String) -> Result<Vec<Violation>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;
    let res = client
        .get(violations_api_url())
        .query(&[("username", &username)])
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Violations API returned {}", res.status()));
    }

    let parsed: ViolationsResponse = res.json().await.map_err(|err| err.to_string())?;
    Ok(parsed.violations)
}
