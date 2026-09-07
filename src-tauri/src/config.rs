pub const API_BASE: &str = "https://lemicraft.ru/api";

/// Players connect here, not lemicraft.ru itself — separate box running Velocity, different IP
pub const SERVER_HOST: &str = "proxy.lemicraft.ru";
pub const SERVER_PORT: u16 = 25565;

/// Bare domain the SRV record for SERVER_HOST is queried against
pub const SERVER_SRV_DOMAIN: &str = "lemicraft.ru";

pub const MC_VERSION: &str = "1.21.10";
pub const FABRIC_LOADER: &str = "0.18.4";
