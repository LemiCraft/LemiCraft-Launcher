pub const API_BASE: &str = "https://lemicraft.ru/api";
pub const WEBSITE_URL: &str = "https://lemicraft.ru";

/// Players connect here, not lemicraft.ru itself — separate box running Velocity, different IP
pub const SERVER_HOST: &str = "proxy.lemicraft.ru";
pub const SERVER_PORT: u16 = 25565;

/// Bare domain the SRV record for SERVER_HOST is queried against
pub const SERVER_SRV_DOMAIN: &str = "lemicraft.ru";

pub const MC_VERSION: &str = "26.2";
pub const FABRIC_LOADER: &str = "0.19.5";

/// Discord Application ID for Rich Presence; empty disables the feature
pub const DISCORD_CLIENT_ID: &str = "1469639239099744317";

/// How stale an on-disk response cache may be and still serve as a network-down fallback
pub const FALLBACK_CACHE_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);
