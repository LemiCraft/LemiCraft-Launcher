import { reactive } from 'vue';

export const updateState = reactive({
  available: false,
  info: null,
  downloading: false,
  installing: false,
  percent: null,
  bytes: 0,
  error: '',
});

export async function checkForUpdate() {
  if (!window.__TAURI_INTERNALS__) return;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const { getVersion } = await import('@tauri-apps/api/app');
    const currentVersion = await getVersion();
    const info = await invoke('check_for_update', { currentVersion });
    if (info) {
      updateState.info = info;
      updateState.available = true;
    }
  } catch (err) {
    console.error('update check failed:', err);
  }
}

let unlistenProgress = null;

export async function startUpdateDownload() {
  if (!updateState.info || updateState.downloading) return;
  updateState.downloading = true;
  updateState.error = '';
  updateState.percent = null;
  updateState.bytes = 0;

  const { invoke } = await import('@tauri-apps/api/core');
  if (!unlistenProgress) {
    const { listen } = await import('@tauri-apps/api/event');
    unlistenProgress = await listen('update-progress', (event) => {
      updateState.percent = event.payload.percent;
      updateState.bytes = event.payload.bytes;
    });
  }

  try {
    await invoke('download_and_install_update', {
      downloadUrl: updateState.info.download_url,
      sha256: updateState.info.sha256,
    });
    updateState.installing = true;
  } catch (err) {
    updateState.error = String(err);
  } finally {
    updateState.downloading = false;
  }
}

export function dismissUpdate() {
  if (updateState.info?.is_required || updateState.downloading || updateState.installing) return;
  updateState.available = false;
}
