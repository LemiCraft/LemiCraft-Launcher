import { reactive } from 'vue';
import { showProgress, hideProgress } from './progressToast.js';

export const modsState = reactive({
  loaded: false,
  loading: false,
  error: '',
  catalog: [],
  installed: {},
  individualMods: {},
  packModIds: [],
  officialPack: null,
  installedOfficialVersion: null,
  busyIds: [],
  applying: false,
  pendingImportCode: null,
  highlightPackInstall: false,
  installedImportModIds: [],
});

// Set when the Home "no mods installed" banner sends the user here — ModsView briefly
// pulses the pack install button, then clears this itself once the animation has played
export function triggerPackHighlight() {
  modsState.highlightPackInstall = true;
}

// Local-only check (no network) for the Home banner: real, non-dependency mods installed?
export async function hasNoMods() {
  const [info, packVersion, importIds] = await Promise.all([
    withInvoke((invoke) => invoke('get_installed_mods')),
    withInvoke((invoke) => invoke('get_installed_official_pack_version')),
    withInvoke((invoke) => invoke('get_installed_import_bundle')),
  ]);
  const realMods = Object.keys(info?.individual ?? {}).filter((id) => id !== 'fabric-api');
  return realMods.length === 0 && !packVersion && !importIds?.length;
}

async function withInvoke(fn) {
  if (!window.__TAURI_INTERNALS__) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return fn(invoke);
}

let listenersStarted = false;

export async function startModsProgressListener() {
  if (listenersStarted || !window.__TAURI_INTERNALS__) return;
  listenersStarted = true;
  const { listen } = await import('@tauri-apps/api/event');
  await listen('mods-progress', (event) => {
    const p = event.payload;
    if (p.stage === 'downloading-pack') {
      showProgress('Скачиваю сборку...');
    } else if (p.stage === 'extracting-pack') {
      showProgress('Распаковываю сборку...');
    } else if (p.total) {
      const name = modsState.catalog.find((m) => m.id === p.id)?.name || p.id;
      showProgress(`${p.current}/${p.total} — ${name}`, Math.round((p.current / p.total) * 100));
    } else {
      showProgress('Применяю...');
    }
  });
}

let deepLinkListenerStarted = false;

export async function startDeepLinkListener() {
  if (deepLinkListenerStarted || !window.__TAURI_INTERNALS__) return;
  deepLinkListenerStarted = true;
  const { listen } = await import('@tauri-apps/api/event');
  await listen('deep-link-url', (event) => {
    const match = /^lemicraft:\/\/import\/(.+)$/.exec(event.payload ?? '');
    if (match) modsState.pendingImportCode = decodeURIComponent(match[1]);
  });
}

export async function refreshInstalledMods() {
  const info = (await withInvoke((invoke) => invoke('get_installed_mods'))) ?? { individual: {}, pack_ids: [] };
  modsState.individualMods = info.individual;
  modsState.packModIds = info.pack_ids;
  modsState.installed = { ...Object.fromEntries(info.pack_ids.map((id) => [id, null])), ...info.individual };
}

// A mod that's only present via the official pack has no real per-id file ownership —
// it can't be uninstalled on its own (see uninstall_mod), only alongside the whole pack
export function isPackOnly(id) {
  return modsState.packModIds.includes(id) && !(id in modsState.individualMods);
}

export async function loadModCatalog({ force = false } = {}) {
  if (!window.__TAURI_INTERNALS__) return;
  if (modsState.loaded && !force) return;

  if (!modsState.loaded) {
    try {
      const cached = await withInvoke((invoke) => invoke('get_cached_mod_catalog'));
      if (cached?.length) modsState.catalog = cached;
    } catch {}
  }

  if (modsState.catalog.length === 0) modsState.loading = true;
  modsState.error = '';
  try {
    const [catalog] = await Promise.all([withInvoke((invoke) => invoke('get_mod_catalog')), refreshInstalledMods()]);
    modsState.catalog = catalog ?? [];
  } catch (err) {
    modsState.error = String(err);
  } finally {
    modsState.loading = false;
    modsState.loaded = true;
  }
}

export async function loadOfficialPack() {
  const [pack, installedVersion] = await Promise.all([
    withInvoke((invoke) => invoke('get_official_pack')),
    withInvoke((invoke) => invoke('get_installed_official_pack_version')),
  ]);
  modsState.officialPack = pack ?? null;
  modsState.installedOfficialVersion = installedVersion ?? null;
}

export async function loadInstalledImportBundle() {
  const ids = await withInvoke((invoke) => invoke('get_installed_import_bundle'));
  modsState.installedImportModIds = ids ?? [];
}

export async function applyOfficialPack() {
  const pack = modsState.officialPack;
  if (!pack || modsState.applying) return;
  modsState.applying = true;
  showProgress('Скачиваю сборку...');
  try {
    await withInvoke((invoke) =>
      invoke('apply_official_pack', {
        downloadUrl: pack.download_url,
        version: pack.version,
        minimalMods: pack.minimal_mods,
        fileSize: pack.file_size,
      }),
    );
    modsState.installedOfficialVersion = pack.version;
    await refreshInstalledMods();
  } finally {
    modsState.applying = false;
    hideProgress();
  }
}

export async function uninstallOfficialPack() {
  if (modsState.applying) return;
  modsState.applying = true;
  try {
    await withInvoke((invoke) => invoke('uninstall_official_pack'));
    modsState.installedOfficialVersion = null;
    await refreshInstalledMods();
  } finally {
    modsState.applying = false;
  }
}

export async function installModIds(ids) {
  if (modsState.applying || ids.length === 0) return;
  modsState.applying = true;
  modsState.busyIds = ids;
  showProgress('Применяю...');
  try {
    await withInvoke((invoke) => invoke('install_mods', { ids }));
    await refreshInstalledMods();
  } finally {
    modsState.applying = false;
    modsState.busyIds = [];
    hideProgress();
  }
}

export async function uninstallModId(id) {
  if (modsState.applying) return;
  modsState.applying = true;
  modsState.busyIds = [id];
  try {
    await withInvoke((invoke) => invoke('uninstall_mod', { id }));
    await refreshInstalledMods();
  } finally {
    modsState.applying = false;
    modsState.busyIds = [];
  }
}

export async function previewImportCode(code) {
  return withInvoke((invoke) => invoke('preview_import_code', { code }));
}

// One download for the whole code instead of an export+download per mod (rate limits); tracked as a unit like the official pack,
// so mods from an import aren't individually toggleable after
export async function applyImportCode(code, ids) {
  if (modsState.applying || ids.length === 0) return;
  modsState.applying = true;
  modsState.busyIds = ids;
  showProgress('Скачиваю набор...');
  try {
    await withInvoke((invoke) => invoke('apply_import_bundle', { code, modIds: ids }));
    modsState.installedImportModIds = ids;
    await refreshInstalledMods();
  } finally {
    modsState.applying = false;
    modsState.busyIds = [];
    hideProgress();
  }
}

export async function uninstallImportBundle() {
  if (modsState.applying) return;
  modsState.applying = true;
  try {
    await withInvoke((invoke) => invoke('uninstall_import_bundle'));
    modsState.installedImportModIds = [];
    await refreshInstalledMods();
  } finally {
    modsState.applying = false;
  }
}
