import { reactive } from 'vue';

let accountRevision = 0;

export const accountStore = reactive({
  username: null,
  uuid: null,
  skinUrl: null,
  provider: null,
  loggedIn: false,
});

// `info` is what the backend reports as the current account (AccountInfo), or null for a guest
export async function applyAccount(info) {
  const revision = ++accountRevision;
  if (info) {
    // keep the shown head on a plain refresh, so it does not blink
    const sameAccount = accountStore.uuid === info.uuid && accountStore.provider === info.provider;
    accountStore.username = info.username;
    accountStore.uuid = info.uuid;
    accountStore.provider = info.provider;
    accountStore.loggedIn = true;
    if (!sameAccount) accountStore.skinUrl = null;

    if (info.skin_url) {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const skinUrl = await invoke('fetch_skin_data_uri', { url: info.skin_url, force: false });
        if (revision === accountRevision) accountStore.skinUrl = skinUrl;
      } catch (err) {
        console.error('failed to inline skin as data URI:', err);
        if (revision === accountRevision) accountStore.skinUrl = info.skin_url;
      }
    }

    // The account profile endpoint can return an empty default texture. Prefer the active skin
    // from the launcher's skin API when one is available, using its cache first for fast startup.
    void loadManagedProfileSkin(info, revision);
  } else {
    accountStore.username = null;
    accountStore.uuid = null;
    accountStore.skinUrl = null;
    accountStore.provider = null;
    accountStore.loggedIn = false;
  }
}

async function loadManagedProfileSkin(info, revision) {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  const isElyBy = info.provider === 'Ely.by';
  const commands = isElyBy
    ? ['get_cached_elyby_skins', 'get_elyby_skins']
    : ['get_cached_current_account_skins', 'get_current_account_skins'];

  for (const command of commands) {
    try {
      const items = await invoke(command);
      const active = items?.find((skin) => skin.isActive ?? skin.is_active);
      const url = active?.fileUrl ?? active?.skin_url;
      if (!url) continue;

      const skinUrl = await invoke('fetch_skin_data_uri', { url, force: false });
      if (revision === accountRevision && accountStore.uuid === info.uuid && accountStore.provider === info.provider) {
        accountStore.skinUrl = skinUrl;
      }
    } catch {
      // Fall back to the profile skin URL if the launcher skin service is unavailable.
    }
  }
}

export async function refreshAccount() {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  await applyAccount(await invoke('get_current_account'));
}
