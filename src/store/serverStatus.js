import { reactive } from 'vue';

// Module-level singleton so HomeView/PlayHero remounting on tab switch
// doesn't lose the last real ping
export const serverStatus = reactive({
  loaded: false,
  online: false,
  players: null,
  maxPlayers: null,
  version: '',
  ping: null,
});

let pollTimer = null;
let started = false;

function applyStatus(status) {
  serverStatus.online = status.online;
  serverStatus.players = status.players_online;
  serverStatus.maxPlayers = status.players_max;
  serverStatus.version = status.version;
  serverStatus.ping = Math.round(status.latency_ms);
}

export async function fetchServerStatus() {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');

  // Instant paint from disk cache on cold mount; the live ping below still runs regardless
  if (!serverStatus.loaded) {
    try {
      const cached = await invoke('get_cached_server_status');
      if (cached) applyStatus(cached);
    } catch {}
  }

  try {
    const status = await invoke('get_server_status');
    applyStatus(status);
  } catch (err) {
    console.error('server status ping failed:', err);
    serverStatus.online = false;
  } finally {
    serverStatus.loaded = true;
  }
}

// Conservative interval — the proxy's anti-bot layer appears to penalize frequent probing
export function startServerStatusPolling() {
  if (started) return;
  started = true;
  fetchServerStatus();
  pollTimer = setInterval(fetchServerStatus, 120000);
}
