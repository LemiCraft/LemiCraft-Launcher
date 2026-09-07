import { reactive } from 'vue';

export const rulesState = reactive({
  doc: null,
  loaded: false,
  loading: false,
  error: false,
  pendingAnchors: [],
  highlightedRuleIds: [],
});

const CACHE_TTL_MS = 10 * 60 * 1000;
let lastFetchedAt = 0;

export async function fetchRules({ force = false } = {}) {
  if (!window.__TAURI_INTERNALS__) {
    rulesState.loaded = true;
    return;
  }

  const { invoke } = await import('@tauri-apps/api/core');

  if (!rulesState.loaded && !rulesState.doc) {
    try {
      const cached = await invoke('get_cached_rules');
      if (cached) rulesState.doc = cached;
    } catch {}
  }

  const isFresh = rulesState.loaded && Date.now() - lastFetchedAt < CACHE_TTL_MS;
  if (isFresh && !force) return;

  if (!rulesState.doc) rulesState.loading = true;
  try {
    rulesState.doc = await invoke('get_rules');
    rulesState.error = false;
    lastFetchedAt = Date.now();
  } catch (err) {
    console.error('failed to fetch rules:', err);
    if (!rulesState.doc) rulesState.error = true;
  } finally {
    rulesState.loading = false;
    rulesState.loaded = true;
  }
}
