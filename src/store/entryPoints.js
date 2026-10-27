import { reactive } from 'vue';

// How the server can be reached (directly or through a relay) and which way answers best
export const entryPoints = reactive({
  probe: null,
  checking: false,
});

const STALE_AFTER_SECS = 10 * 60;

export async function runEntryProbe() {
  if (!window.__TAURI_INTERNALS__ || entryPoints.checking) return;
  entryPoints.checking = true;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    entryPoints.probe = await invoke('probe_entry_points');
  } catch (err) {
    console.error('entry point check failed:', err);
  } finally {
    entryPoints.checking = false;
  }
}

// The last result from disk, no network
export async function loadCachedEntryPoints() {
  if (!window.__TAURI_INTERNALS__ || entryPoints.probe) return;
  const { invoke } = await import('@tauri-apps/api/core');
  entryPoints.probe = await invoke('get_cached_entry_points');
}

// Shows the last result right away, and checks again when it is old
export async function initEntryPoints() {
  await loadCachedEntryPoints();
  const age = Date.now() / 1000 - (entryPoints.probe?.checked_at ?? 0);
  if (age > STALE_AFTER_SECS) runEntryProbe();
}

export function entryState(point) {
  if (point.score == null) return { text: 'Не отвечает', level: 'bad', hint: 'Сервер по этому пути не ответил' };
  if (point.loss >= 0.2) return { text: 'Нестабильно', level: 'bad', hint: 'Много запросов потерялось: подключение может обрываться' };
  if (point.loss > 0) return { text: 'Есть сбои', level: 'warn', hint: 'Часть запросов потерялась' };
  // a spread that is large for this delay, not just for a long way
  if ((point.jitter_ms ?? 0) > Math.max(30, (point.ping_ms ?? 0) * 0.3)) return { text: 'Колебания', level: 'warn', hint: 'Пинг заметно скачет: в игре возможны рывки' };
  return { text: 'Стабильно', level: 'good', hint: 'Пинг ровный, запросы не теряются' };
}
