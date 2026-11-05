import { reactive } from 'vue';

// The signed-in player's profile (same data as the site's profile page)
export const profileState = reactive({
  profile: null,
  loading: false,
  error: '',
});

const REFRESH_AFTER_MS = 60 * 1000;
let lastLoadedAt = 0;
let request = 0;

// Shows the cached profile at once, then asks the site; `force` asks even when the data is recent
export async function loadProfile({ force = false } = {}) {
  if (!window.__TAURI_INTERNALS__) return;
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
