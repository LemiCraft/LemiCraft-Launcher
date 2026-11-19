import { watch } from 'vue';
import { newsState, fetchNews } from './news.js';
import { serverStatus, startServerStatusPolling } from './serverStatus.js';

// Tells the player about a new post and about the server coming back while the launcher is not in front
// (the backend drops the notification when the window is, and when notifications are off)
const NEWS_CHECK_MS = 5 * 60 * 1000;
const LAST_NEWS_KEY = 'lc-last-news';
let started = false;

export async function notify(title, body, force = false) {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('show_notification', { title, body, force });
  } catch (err) {
    console.error('failed to show a notification:', err);
  }
}

function remembered() {
  try {
    return JSON.parse(localStorage.getItem(LAST_NEWS_KEY) ?? 'null');
  } catch {
    return null;
  }
}

function remember(item) {
  try {
    localStorage.setItem(LAST_NEWS_KEY, JSON.stringify({ id: item.id, at: Date.parse(item.rawDate) || 0 }));
  } catch {
    // without storage every start simply begins from what is on the site now
  }
}

export function startNotifyWatcher() {
  if (started || !window.__TAURI_INTERNALS__) return;
  started = true;

  // the feed asks only while it is on screen; this keeps asking on the other pages and when minimized
  setInterval(() => fetchNews(), NEWS_CHECK_MS);
  watch(
    () => newsState.items[0],
    (top) => {
      if (!top) return;
      const last = remembered();
      const at = Date.parse(top.rawDate) || 0;
      // newer than the last one seen, not merely another one (a deleted post moves an older one to the top)
      if (last && last.id !== top.id && at > last.at) notify('Новость LemiCraft', String(top.title ?? '').slice(0, 140));
      if (!last || at >= last.at) remember(top);
    },
    { immediate: true },
  );

  startServerStatusPolling();
  let wasOffline = false;
  watch(
    () => [serverStatus.loaded, serverStatus.online],
    ([loaded, online]) => {
      if (!loaded) return;
      if (online && wasOffline) notify('Сервер снова онлайн', 'LemiCraft доступен, можно заходить');
      wasOffline = !online;
    },
  );
}
