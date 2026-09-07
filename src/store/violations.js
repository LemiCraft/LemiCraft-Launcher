import { reactive, watch } from 'vue';
import { accountStore } from './account.js';

export const violationsState = reactive({
  items: [],
  loaded: false,
  loading: false,
});

export async function fetchViolations() {
  if (!window.__TAURI_INTERNALS__ || !accountStore.loggedIn) {
    violationsState.items = [];
    violationsState.loaded = true;
    return;
  }
  violationsState.loading = true;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    violationsState.items = await invoke('get_violations', { username: accountStore.username });
  } catch (err) {
    console.error('failed to fetch violations:', err);
    violationsState.items = [];
  } finally {
    violationsState.loading = false;
    violationsState.loaded = true;
  }
}

let watcherStarted = false;

export function startViolationsWatcher() {
  if (watcherStarted) return;
  watcherStarted = true;
  watch(() => [accountStore.loggedIn, accountStore.username], () => fetchViolations(), { immediate: true });
}
