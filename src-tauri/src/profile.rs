// The player's profile page in the launcher: the same data as the profile on the site. The site's launcher
// endpoint gives all of it for the signed-in account (the token says whose profile it is); while that
// endpoint is not there, the public data of the player (roles, ban) is shown without statistics.
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

fn launcher_profile_url() -> String {
    format!("{}/launcher/profile", crate::config::API_BASE)
}

fn public_player_url(nick: &str) -> String {
    format!("{}/players/{}", crate::config::API_BASE, nick)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRole {
    pub id: String,
    pub name: String,
    /// Twemoji file name without the extension, the picture is at /api/emoji/{code}.svg
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub colors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStats {
    #[serde(default)]
    pub registered: Option<String>,
    #[serde(default)]
    pub last_login: Option<String>,
    /// Milliseconds
    #[serde(default)]
    pub playtime: Option<u64>,
    #[serde(default)]
    pub active_playtime: Option<u64>,
    #[serde(default)]
    pub sessions: Option<u64>,
    #[serde(default)]
    pub deaths: Option<u64>,
    #[serde(default)]
    pub mob_kills: Option<u64>,
    #[serde(default)]
    pub player_kills: Option<u64>,
    #[serde(default)]
    pub ping: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub nick: String,
    pub uuid: String,
    #[serde(default)]
    pub banned: bool,
    #[serde(default)]
    pub ban_reason: Option<String>,
    #[serde(default)]
    pub ban_permanent: bool,
    #[serde(default)]
    pub ban_ends_at: Option<String>,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub last_seen: Option<String>,
    #[serde(default)]
    pub playtime_rank: Option<u32>,
    #[serde(default)]
    pub roles: Vec<ProfileRole>,
    /// None when the statistics are not available (the stats service is down, or the site has no launcher endpoint yet)
    #[serde(default)]
    pub stats: Option<ProfileStats>,
    /// True when the profile came from the launcher endpoint of the site; false for the public data only
    #[serde(default)]
    pub complete: bool,
}

#[derive(Deserialize)]
struct Wire {
    success: bool,
    #[serde(flatten)]
    profile: Profile,
}

/// What an answer of the launcher endpoint means
#[derive(Debug, PartialEq)]
enum Answer {
    Profile,
    /// The endpoint is not there (yet)
    NotDeployed,
    Failed,
}

fn classify(status: u16) -> Answer {
    match status {
        200..=299 => Answer::Profile,
        404 | 405 | 501 => Answer::NotDeployed,
        _ => Answer::Failed,
    }
}

fn parse_launcher_answer(body: &[u8]) -> Result<Profile, String> {
    let wire: Wire = serde_json::from_slice(body).map_err(|err| format!("Не удалось разобрать профиль: {err}"))?;
    if !wire.success {
        return Err("Сайт не отдал профиль".to_string());
    }
    Ok(Profile { complete: true, ..wire.profile })
}

/// The profile from the two public answers of the site (the player and, when it came, the roles)
fn profile_from_public(player: &serde_json::Value, roles: Option<&serde_json::Value>) -> Result<Profile, String> {
    let nick = player.get("nick").and_then(|v| v.as_str()).ok_or("Игрок не найден")?;
    let text = |key: &str| player.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let roles = roles
        .and_then(|v| v.get("roles"))
        .and_then(|v| serde_json::from_value::<Vec<ProfileRole>>(v.clone()).ok())
        .unwrap_or_default();
    Ok(Profile {
        nick: nick.to_string(),
        uuid: text("uuid").unwrap_or_default(),
        banned: player.get("banned").and_then(|v| v.as_bool()).unwrap_or(false),
        ban_reason: text("banReason"),
        ban_permanent: player.get("banPermanent").and_then(|v| v.as_bool()).unwrap_or(false),
        ban_ends_at: text("banEndsAt"),
        online: false,
        last_seen: None,
        playtime_rank: None,
        roles,
        stats: None,
        complete: false,
    })
}

fn normalized(uuid: &str) -> String {
    uuid.replace('-', "").to_lowercase()
}

#[derive(Serialize, Deserialize)]
struct Cached {
    uuid: String,
    profile: Profile,
}

// Kept for the first paint only, and only for the account it belongs to: the next login on this machine
// must never see it
fn read_cache(path: &Path, uuid: &str) -> Option<Profile> {
    let cached: Cached = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    (normalized(&cached.uuid) == normalized(uuid)).then_some(cached.profile)
}

fn write_cache(path: &Path, uuid: &str, profile: &Profile) {
    if let Ok(json) = serde_json::to_string(&Cached { uuid: uuid.to_string(), profile: profile.clone() }) {
        let _ = std::fs::write(path, json);
    }
}

fn cache_path() -> std::path::PathBuf {
    crate::game::game_dir().join("profile_cache.json")
}

pub(crate) fn clear_cache() {
    let _ = std::fs::remove_file(cache_path());
}

#[tauri::command]
pub async fn get_cached_profile() -> Option<Profile> {
    tauri::async_runtime::spawn_blocking(|| {
        let (uuid, _) = crate::auth::current_identity()?;
        read_cache(&cache_path(), &uuid)
    })
    .await
    .unwrap_or(None)
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build().map_err(|err| err.to_string())
}

// None: the site has no launcher endpoint for the profile yet
fn from_launcher_endpoint(client: &reqwest::blocking::Client) -> Result<Option<Profile>, String> {
    let url = launcher_profile_url();
    let res = crate::site_auth::send_authed(client, || client.get(&url))?;
    let status = res.status();
    match classify(status.as_u16()) {
        Answer::Profile => {
            let body = res.bytes().map_err(|err| err.to_string())?;
            parse_launcher_answer(&body).map(Some)
        }
        Answer::NotDeployed => {
            // the endpoint answers 404 itself when the player is not on the whitelist; a missing route says nothing of the kind
            let body = res.text().unwrap_or_default();
            eprintln!("profile: the site answered {status}: {body}");
            if is_unknown_player(&body) {
                Err("Игрока нет в белом списке сервера".to_string())
            } else {
                Ok(None)
            }
        }
        Answer::Failed if status.as_u16() == 429 => Err("Слишком много запросов, попробуйте через минуту".to_string()),
        Answer::Failed => Err(format!("Не удалось загрузить профиль ({status})")),
    }
}

fn is_unknown_player(body: &str) -> bool {
    body.contains("Игрок не найден")
}

fn from_public_data(client: &reqwest::blocking::Client, nick: &str) -> Result<Profile, String> {
    let get = |url: String| -> Result<serde_json::Value, String> {
        let res = client.get(url).send().map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            return Err(format!("({})", res.status()));
        }
        res.json().map_err(|err| err.to_string())
    };
    let player = get(public_player_url(nick)).map_err(|err| format!("Не удалось загрузить профиль {err}"))?;
    // the roles are a bonus: a profile without them is better than none
    let roles = get(format!("{}/roles", public_player_url(nick))).ok();
    profile_from_public(&player, roles.as_ref())
}

#[tauri::command]
pub async fn get_profile() -> Result<Profile, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (uuid, nick) = crate::auth::current_identity().ok_or("Войдите в аккаунт")?;
        let client = client()?;
        let profile = match from_launcher_endpoint(&client)? {
            Some(profile) => profile,
            None => from_public_data(&client, &nick)?,
        };
        write_cache(&cache_path(), &uuid, &profile);
        Ok(profile)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r##"{
        "success": true, "nick": "KOTOKOPOLb", "uuid": "21f2cae3-0176-4a9b-9de9-bae5ad93750c",
        "banned": false, "banReason": null, "banPermanent": false, "banEndsAt": null,
        "online": false, "lastSeen": "2026-11-04T10:12:00.000Z", "playtimeRank": 43,
        "roles": [{"id": "1", "name": "Комиссар", "emoji": "1f6c3", "colors": ["#2136AC", "#1F2753"]}, {"id": "2", "name": "Бета-тестер", "emoji": null, "colors": []}],
        "stats": {"registered": "2026-02-24T12:00:00.000Z", "lastLogin": "2026-11-03T20:50:00.000Z", "playtime": 893660000, "activePlaytime": 800000000,
                  "sessions": 726, "deaths": 131, "mobKills": 1180, "playerKills": 306, "ping": 95.4, "somethingNew": 1},
        "somethingElse": true
    }"##;

    #[test]
    fn the_launcher_answer_is_read_in_full_and_marked_complete() {
        let profile = parse_launcher_answer(FULL.as_bytes()).unwrap();
        assert_eq!((profile.nick.as_str(), profile.playtime_rank, profile.complete, profile.online), ("KOTOKOPOLb", Some(43), true, false));
        assert_eq!(profile.roles.len(), 2);
        assert_eq!(profile.roles[0], ProfileRole { id: "1".into(), name: "Комиссар".into(), emoji: Some("1f6c3".into()), colors: vec!["#2136AC".into(), "#1F2753".into()] });
        assert_eq!(profile.roles[1].emoji, None);
        let stats = profile.stats.unwrap();
        assert_eq!((stats.playtime, stats.sessions, stats.deaths, stats.mob_kills, stats.player_kills), (Some(893_660_000), Some(726), Some(131), Some(1180), Some(306)));
        assert_eq!(stats.ping, Some(95.4));
        assert_eq!(stats.last_login.as_deref(), Some("2026-11-03T20:50:00.000Z"));
    }

    #[test]
    fn the_answer_of_the_live_site_is_read() {
        let body = r##"{"success":true,"nick":"KOTOKOPOLb","uuid":"21f2cae3-0176-4a9b-9de9-bae5ad93750c","banned":false,"banReason":null,"banPermanent":false,"banEndsAt":null,"online":false,"lastSeen":"2026-11-04T16:50:23.354Z","playtimeRank":42,"roles":[{"id":"1447702607475904593","name":"Комиссар","emoji":"1f6c3","colors":["#2136AC","#1F2753"]}],"stats":{"registered":"2026-02-24T15:35:23.359Z","lastLogin":"2026-11-04T16:50:23.354Z","playtime":894070530,"activePlaytime":746926133,"sessions":726,"deaths":131,"mobKills":1180,"playerKills":306,"ping":95}}"##;
        let profile = parse_launcher_answer(body.as_bytes()).unwrap();
        assert_eq!((profile.playtime_rank, profile.last_seen.as_deref()), (Some(42), Some("2026-11-04T16:50:23.354Z")));
        assert_eq!(profile.stats.unwrap().ping, Some(95.0));
    }

    #[test]
    fn a_missing_stats_block_stays_missing_instead_of_turning_into_zeros() {
        let body = r#"{"success": true, "nick": "Steve", "uuid": "u", "stats": null}"#;
        let profile = parse_launcher_answer(body.as_bytes()).unwrap();
        assert_eq!(profile.stats, None);
        assert!(profile.complete && profile.roles.is_empty());
        let body = r#"{"success": true, "nick": "Steve", "uuid": "u", "stats": {"sessions": 3}}"#;
        let stats = parse_launcher_answer(body.as_bytes()).unwrap().stats.unwrap();
        assert_eq!((stats.sessions, stats.playtime, stats.deaths), (Some(3), None, None));
    }

    #[test]
    fn a_refusal_or_garbage_is_an_error() {
        assert!(parse_launcher_answer(br#"{"success": false, "nick": "x", "uuid": "u"}"#).is_err());
        assert!(parse_launcher_answer(b"<html>").is_err());
        assert!(parse_launcher_answer(br#"{"success": true}"#).is_err());
    }

    #[test]
    fn the_status_of_the_answer_tells_whether_the_endpoint_is_there() {
        assert_eq!(classify(200), Answer::Profile);
        for missing in [404, 405, 501] {
            assert_eq!(classify(missing), Answer::NotDeployed);
        }
        for failed in [400, 401, 403, 429, 500, 502, 503] {
            assert_eq!(classify(failed), Answer::Failed, "{failed}");
        }
    }

    #[test]
    fn a_404_of_the_player_is_told_from_a_missing_route() {
        assert!(is_unknown_player(r#"{"statusCode":404,"statusMessage":"Not Found","message":"Игрок не найден"}"#));
        assert!(!is_unknown_player(r#"{"statusCode":404,"statusMessage":"Not Found","message":"Page not found: /api/launcher/profile"}"#));
    }

    #[test]
    fn the_public_data_makes_a_profile_without_statistics() {
        let player = serde_json::json!({"nick": "KOTOKOPOLb", "uuid": "21f2cae3-0176-4a9b-9de9-bae5ad93750c", "serviceId": 0, "banned": true, "banReason": "тест", "banPermanent": false, "banEndsAt": "2026-12-01T00:00:00Z"});
        let roles = serde_json::json!({"roles": [{"id": "1", "name": "Лемимист", "emoji": "1f4ef", "colors": ["#F1C40F"]}]});
        let profile = profile_from_public(&player, Some(&roles)).unwrap();
        assert_eq!((profile.nick.as_str(), profile.banned, profile.ban_reason.as_deref(), profile.complete), ("KOTOKOPOLb", true, Some("тест"), false));
        assert_eq!((profile.roles.len(), profile.stats.is_none(), profile.playtime_rank), (1, true, None));
        // roles that did not come, or came in a shape nobody expected, do not spoil the profile
        assert!(profile_from_public(&player, None).unwrap().roles.is_empty());
        assert!(profile_from_public(&player, Some(&serde_json::json!({"roles": "nope"}))).unwrap().roles.is_empty());
        assert!(profile_from_public(&serde_json::json!({"error": true}), None).is_err());
    }

    #[test]
    fn the_cache_belongs_to_one_account() {
        let dir = std::env::temp_dir().join(format!("lc-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile_cache.json");
        let profile = parse_launcher_answer(FULL.as_bytes()).unwrap();
        write_cache(&path, "21F2CAE3-0176-4A9B-9DE9-BAE5AD93750C", &profile);
        // the same account, written with or without dashes and in any case
        assert_eq!(read_cache(&path, "21f2cae301764a9b9de9bae5ad93750c"), Some(profile.clone()));
        // another account gets nothing
        assert_eq!(read_cache(&path, "069a79f4-44e9-4726-a5be-fca90e38aaf5"), None);
        assert_eq!(read_cache(&dir.join("missing.json"), "x"), None);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(read_cache(&path, "21f2cae3-0176-4a9b-9de9-bae5ad93750c"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
