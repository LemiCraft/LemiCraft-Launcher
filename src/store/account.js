import { reactive } from 'vue';

export const accountStore = reactive({
  username: null,
  uuid: null,
  skinUrl: null,
  provider: null,
  loggedIn: false,
});

export async function refreshAccount() {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  const info = await invoke('get_current_account');
  if (info) {
    accountStore.username = info.username;
    accountStore.uuid = info.uuid;
    accountStore.provider = info.provider;
    accountStore.loggedIn = true;
    if (info.skin_url) {
      try {
        accountStore.skinUrl = await invoke('fetch_skin_data_uri', { url: info.skin_url, force: false });
      } catch (err) {
        console.error('failed to inline skin as data URI:', err);
        accountStore.skinUrl = info.skin_url;
      }
    } else {
      accountStore.skinUrl = null;
    }
  } else {
    accountStore.username = null;
    accountStore.uuid = null;
    accountStore.skinUrl = null;
    accountStore.provider = null;
    accountStore.loggedIn = false;
  }
}
