import { reactive } from 'vue';

// The signed-in player's profile (same data as the site's profile page)
export const profileState = reactive({
  profile: null,
  loading: false,
  error: '',
});

// Whose profile the page shows: '' is the signed-in player's own
export const profileView = reactive({ nick: '' });

// Everyone on the server now, for the list on the play card
export const onlineState = reactive({ players: [], loadedAt: 0 });
const ONLINE_FRESH_MS = 30 * 1000;

export async function loadOnlinePlayers() {
  if (!window.__TAURI_INTERNALS__ || Date.now() - onlineState.loadedAt < ONLINE_FRESH_MS) return;
  onlineState.loadedAt = Date.now();
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    onlineState.players = await invoke('get_online_players');
  } catch (err) {
    onlineState.loadedAt = 0;
    console.error('failed to load the online players:', err);
  }
}

const REFRESH_AFTER_MS = 60 * 1000;
let lastLoadedAt = 0;
let request = 0;

// Shows the cached profile at once, then asks the site; `force` asks even when the data is recent
export async function loadProfile({ force = false } = {}) {
  if (!window.__TAURI_INTERNALS__) return;
  // checked before the request counter moves: a call that does nothing must not cancel one that is running
  if (!force && profileState.profile && Date.now() - lastLoadedAt < REFRESH_AFTER_MS) return;
  const { invoke } = await import('@tauri-apps/api/core');
  const mine = ++request;

  if (!profileState.profile) {
    try {
      const cached = await invoke('get_cached_profile');
      if (mine === request && cached && !profileState.profile) profileState.profile = cached;
    } catch {
      // no cache is fine
    }
  }

  if (!force && profileState.profile && Date.now() - lastLoadedAt < REFRESH_AFTER_MS) return;

  profileState.loading = true;
  profileState.error = '';
  try {
    const live = await invoke('get_profile');
    if (mine !== request) return;
    profileState.profile = live;
    lastLoadedAt = Date.now();
  } catch (err) {
    if (mine !== request) return;
    // a shown profile stays; only a page with nothing to show needs the message
    if (!profileState.profile) profileState.error = String(err);
    else console.error('failed to refresh the profile:', err);
  } finally {
    if (mine === request) profileState.loading = false;
  }
}

// The next account must not see this one's profile
export function clearProfile() {
  request++;
  profileState.profile = null;
  profileState.error = '';
  profileState.loading = false;
  lastLoadedAt = 0;
}
