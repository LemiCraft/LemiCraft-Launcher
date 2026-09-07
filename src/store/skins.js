import { reactive } from 'vue';
import { accountStore, refreshAccount } from './account.js';

function isElyBy() {
  return accountStore.provider === 'Ely.by';
}

export const skinsState = reactive({
  loaded: false,
  loading: false,
  items: [],
  error: '',
  elybySessionReady: false,
  elybyLoginPending: false,
});

async function withInvoke(fn) {
  if (!window.__TAURI_INTERNALS__) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return fn(invoke);
}

let listenersStarted = false;

export async function startElybyLoginListeners() {
  if (listenersStarted || !window.__TAURI_INTERNALS__) return;
  listenersStarted = true;
  const { listen } = await import('@tauri-apps/api/event');

  await listen('elyby-login-opened', () => {
    skinsState.elybyLoginPending = true;
  });
  await listen('elyby-login-success', async () => {
    skinsState.elybyLoginPending = false;
    await checkElybySession();
    await loadSkins({ force: true });
  });
  await listen('elyby-login-error', (event) => {
    skinsState.elybyLoginPending = false;
    skinsState.error = typeof event.payload === 'string' ? event.payload : '';
  });
}

export async function checkElybySession() {
  skinsState.elybySessionReady = (await withInvoke((invoke) => invoke('has_elyby_skin_session'))) ?? false;
}

export async function openElybyWebLogin() {
  await withInvoke((invoke) => invoke('open_elyby_web_login'));
}

export async function finishElybyWebLogin() {
  await withInvoke((invoke) => invoke('finish_elyby_web_login'));
  skinsState.elybyLoginPending = false;
  await checkElybySession();
  await loadSkins({ force: true });
}

export async function forgetElybySession() {
  await withInvoke((invoke) => invoke('clear_elyby_skin_session'));
  skinsState.elybySessionReady = false;
  skinsState.items = [];
}

const SKINS_CACHE_TTL_MS = 2 * 60 * 1000;
let lastLoadedKey = null;
let lastLoadedAt = 0;
// Bumped on every loadSkins() call so an earlier, still-in-flight call can detect a newer
// one has started and discard its own (potentially stale) result instead of overwriting it
let loadGeneration = 0;

function mapElyItems(elyItems, dbItems) {
  const dbByElybyId = new Map(dbItems.filter((s) => s.elybyId != null).map((s) => [s.elybyId, s]));
  return elyItems.map((s) => {
    const db = dbByElybyId.get(s.id);
    return {
      id: s.id,
      elybyId: s.id,
      name: db?.name || `Скин #${s.id}`,
      model: s.is_slim ? 'alex' : 'steve',
      fileUrl: db?.fileUrl || s.skin_url,
      thumbnailUrl: db?.thumbnailUrl || s.skin_url,
      isActive: s.isActive,
    };
  });
}

function mapLicenseItems(items) {
  // Skins can carry a non-null elybyId here when the same username was once used through
  // ely.by too — those aren't part of this (license) account's own library.
  return items
    .filter((s) => s.elybyId == null)
    .map((s) => ({
      id: s.id,
      elybyId: s.elybyId,
      name: s.name,
      model: s.model,
      fileUrl: s.fileUrl,
      thumbnailUrl: s.thumbnailUrl,
      isActive: s.isActive,
      addedAt: s.addedAt || null,
    }));
}

export async function loadSkins({ force = false } = {}) {
  if (!window.__TAURI_INTERNALS__ || !accountStore.loggedIn) return;

  const cacheKey = `${accountStore.provider}:${accountStore.username}`;
  const isFresh = skinsState.loaded && lastLoadedKey === cacheKey && Date.now() - lastLoadedAt < SKINS_CACHE_TTL_MS;
  if (isFresh && !force) return;

  const { invoke } = await import('@tauri-apps/api/core');
  const myGeneration = ++loadGeneration;

  if ((!skinsState.loaded || lastLoadedKey !== cacheKey) && !isElyBy()) {
    try {
      const cached = await invoke('get_cached_current_account_skins');
      if (myGeneration !== loadGeneration) return;
      if (cached.length) skinsState.items = mapLicenseItems(cached);
    } catch {}
  }

  if (skinsState.items.length === 0) skinsState.loading = true;
  skinsState.error = '';
  try {
    if (isElyBy()) {
      // Inlined rather than checkElybySession(): that writes elybySessionReady unconditionally,
      // which an older overlapping call could still do after this generation check, clobbering it
      const ready = (await withInvoke((invoke) => invoke('has_elyby_skin_session'))) ?? false;
      if (myGeneration !== loadGeneration) return;
      skinsState.elybySessionReady = ready;
      if (!skinsState.elybySessionReady) {
        skinsState.items = [];
        return;
      }
      const [elyItems, dbItems] = await Promise.all([
        invoke('get_elyby_skins'),
        invoke('get_current_account_skins').catch(() => []),
      ]);
      if (myGeneration !== loadGeneration) return;
      skinsState.items = mapElyItems(elyItems, dbItems);
    } else {
      const items = await invoke('get_current_account_skins');
      if (myGeneration !== loadGeneration) return;
      skinsState.items = mapLicenseItems(items);
    }
  } catch (err) {
    if (myGeneration !== loadGeneration) return;
    skinsState.error = String(err);
  } finally {
    if (myGeneration === loadGeneration) {
      skinsState.loading = false;
      skinsState.loaded = true;
      lastLoadedKey = cacheKey;
      lastLoadedAt = Date.now();
    }
  }
}

export async function applySkin(skin) {
  const { invoke } = await import('@tauri-apps/api/core');
  const ok = isElyBy()
    ? await invoke('wear_elyby_skin', { skinId: skin.elybyId })
    : await invoke('apply_current_account_skin', { skinId: skin.id });
  if (!ok) throw new Error('Сайт не подтвердил смену скина — попробуйте ещё раз');
  await loadSkins({ force: true });
  await refreshAccount();
}

export async function deleteSkin(skin) {
  const { invoke } = await import('@tauri-apps/api/core');
  const ok = isElyBy()
    ? await invoke('delete_elyby_skin', { skinId: skin.elybyId })
    : await invoke('delete_current_account_skin', { skinId: skin.id });
  if (!ok) throw new Error('Сайт не подтвердил удаление скина — попробуйте ещё раз');
  await loadSkins({ force: true });
}

export async function uploadSkin(file, name, model) {
  const { invoke } = await import('@tauri-apps/api/core');
  const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));

  if (isElyBy()) {
    await invoke('upload_elyby_skin', { fileBytes: bytes, fileName: file.name, name, isSlim: model === 'alex' });
  } else {
    await invoke('upload_current_account_skin', { fileBytes: bytes, fileName: file.name, name, model });
  }
  await loadSkins({ force: true });
}
