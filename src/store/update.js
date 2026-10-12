import { reactive } from 'vue';

export const updateState = reactive({
  available: false,
  info: null,
  downloading: false,
  installing: false,
  percent: null,
  bytes: 0,
  error: '',
  fallbackOffered: false,
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

  if (updateState.info.manual) {
    updateState.error = '';
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('open_external', { target: updateState.info.download_url });
      updateState.available = false;
    } catch (err) {
      updateState.error = String(err);
    }
    return;
  }

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

  const inApp = updateState.info.mode === 'in_app';
  try {
    if (inApp) {
      // Doesn't return on success: the launcher restarts into the new version
      await invoke('install_update_in_app');
    } else {
      await invoke('download_and_install_update', {
        downloadUrl: updateState.info.download_url,
        sha256: updateState.info.sha256,
      });
    }
    updateState.installing = true;
  } catch (err) {
    updateState.error = String(err);
    // The in-place update didn't work (read-only folder, manifest not published yet, no network): keep retry and also offer the download page
    if (inApp) updateState.fallbackOffered = true;
  } finally {
    updateState.downloading = false;
  }
}

export async function openUpdatePage() {
  if (!updateState.info) return;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_external', { target: updateState.info.download_url });
  } catch (err) {
    updateState.error = String(err);
  }
}

export function dismissUpdate() {
  if (updateState.info?.is_required || updateState.downloading || updateState.installing) return;
  updateState.available = false;
}
