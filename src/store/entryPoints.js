import { reactive } from 'vue';

// How the server can be reached (directly or through a relay) and which way answers best
export const entryPoints = reactive({
  probe: null,
  checking: false,
  mode: 'quick',
  // requests answered of the requests planned in the running check
  progress: { done: 0, total: 0 },
});

const STALE_AFTER_SECS = 10 * 60;

let listening = false;

async function listenProgress() {
  if (listening) return;
  listening = true;
  try {
    const { listen } = await import('@tauri-apps/api/event');
    await listen('entry-probe-progress', (event) => {
      entryPoints.progress = event.payload;
    });
  } catch (err) {
    listening = false;
    console.error('failed to listen to the entry probe progress:', err);
  }
}

// `mode` is 'quick' or 'deep'
export async function runEntryProbe(mode = 'quick') {
  if (!window.__TAURI_INTERNALS__ || entryPoints.checking) return;
  entryPoints.checking = true;
  entryPoints.mode = mode;
  entryPoints.progress = { done: 0, total: 0 };
  try {
    await listenProgress();
    const { invoke } = await import('@tauri-apps/api/core');
    entryPoints.probe = await invoke('probe_entry_points', { mode });
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
  try {
    entryPoints.probe = await invoke('get_cached_entry_points');
  } catch (err) {
    console.error('failed to read the last entry check:', err);
  }
}

// Shows the last result right away, and checks again when it is old
export async function initEntryPoints() {
  await loadCachedEntryPoints();
  const age = Date.now() / 1000 - (entryPoints.probe?.checked_at ?? 0);
  if (age > STALE_AFTER_SECS) runEntryProbe();
}

export function entryState(point) {
  if (point.score == null) return { text: 'Не отвечает', level: 'bad', hint: point.error || 'Сервер по этому пути не ответил' };
  if (point.loss >= 0.2) return { text: 'Нестабильно', level: 'bad', hint: 'Много запросов потерялось: подключение может обрываться' };
  if (point.loss > 0) return { text: 'Есть сбои', level: 'warn', hint: 'Часть запросов потерялась' };
  const ping = point.ping_ms ?? 0;
  // a spread that is large for this delay, not just for a long way
  if ((point.jitter_ms ?? 0) > Math.max(30, ping * 0.3)) {
    return { text: 'Колебания', level: 'warn', hint: 'Пинг заметно скачет: в игре возможны рывки' };
  }
  if (point.tail_ms != null && point.tail_ms - ping > Math.max(80, ping)) {
    return { text: 'Бывают скачки', level: 'warn', hint: 'Обычно пинг ровный, но изредка подскакивает: возможен одиночный рывок' };
  }
  return { text: 'Стабильно', level: 'good', hint: 'Пинг ровный, запросы не теряются' };
}
