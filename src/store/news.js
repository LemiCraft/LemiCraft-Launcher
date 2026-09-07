import { reactive } from 'vue';

export const newsState = reactive({
  items: [],
  loaded: false,
  loading: false,
  loadingMore: false,
  error: false,
  hasMore: false,
  lastId: null,
});

const CACHE_TTL_MS = 2 * 60 * 1000;
let lastFetchedAt = 0;
// Bumped by every fetchNews() call so a slower-resolving loadMoreNews() from before a
// refresh can detect it's stale and discard its result instead of appending onto a reset list
let requestGeneration = 0;

function relativeDate(iso) {
  const diffMs = Date.now() - new Date(iso).getTime();
  const minutes = Math.floor(diffMs / 60000);
  if (minutes < 1) return 'Только что';
  if (minutes < 60) return `${minutes} мин назад`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} ч назад`;
  const days = Math.floor(hours / 24);
  if (days === 1) return 'вчера';
  if (days < 7) return `${days} дн. назад`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks} нед. назад`;
  const months = Math.floor(days / 30);
  return `${months} мес. назад`;
}

function mapNewsItem(item) {
  return {
    id: item.id,
    title: item.title,
    content: item.content,
    excerpt: item.excerpt || item.title,
    imageUrl: item.imageUrl,
    date: relativeDate(item.date),
    rawDate: item.date,
    tag: item.tag,
    author: item.author,
    authorRole: item.authorRole,
    authorAvatarUrl: item.authorAvatarUrl,
    url: item.url,
  };
}

export async function fetchNews({ force = false } = {}) {
  if (!window.__TAURI_INTERNALS__) {
    newsState.loaded = true;
    return;
  }

  const { invoke } = await import('@tauri-apps/api/core');
  const myGeneration = ++requestGeneration;

  if (!newsState.loaded && newsState.items.length === 0) {
    try {
      const cached = await invoke('get_cached_news');
      if (myGeneration !== requestGeneration) return;
      if (cached?.items?.length) {
        newsState.items = cached.items.map(mapNewsItem);
        newsState.hasMore = cached.hasMore;
        newsState.lastId = cached.lastId;
      }
    } catch {}
  }

  const isFresh = newsState.loaded && Date.now() - lastFetchedAt < CACHE_TTL_MS;
  if (isFresh && !force) return;

  if (newsState.items.length === 0) newsState.loading = true;
  try {
    const page = await invoke('get_news');
    if (myGeneration !== requestGeneration) return;
    newsState.items = page.items.map(mapNewsItem);
    newsState.hasMore = page.hasMore;
    newsState.lastId = page.lastId;
    newsState.error = false;
    lastFetchedAt = Date.now();
  } catch (err) {
    console.error('failed to fetch news:', err);
    if (myGeneration !== requestGeneration) return;
    if (newsState.items.length === 0) newsState.error = true;
  } finally {
    if (myGeneration === requestGeneration) {
      newsState.loading = false;
      newsState.loaded = true;
    }
  }
}

export async function loadMoreNews() {
  if (!window.__TAURI_INTERNALS__ || newsState.loadingMore || !newsState.hasMore || !newsState.lastId) return;
  const myGeneration = requestGeneration;
  newsState.loadingMore = true;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const page = await invoke('get_more_news', { before: newsState.lastId });
    // A fetchNews() (refresh) may have reset the list while this was in flight — discard then
    if (myGeneration !== requestGeneration) return;
    newsState.items.push(...page.items.map(mapNewsItem));
    newsState.hasMore = page.hasMore;
    newsState.lastId = page.lastId;
  } catch (err) {
    console.error('failed to load more news:', err);
  } finally {
    newsState.loadingMore = false;
  }
}
