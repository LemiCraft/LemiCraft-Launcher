import { onMounted, onUnmounted, ref } from 'vue';
import { refreshAccount, accountStore } from '../store/account.js';
import { pushNotification } from '../store/notifications.js';

export function useLogin() {
  const loggingInProvider = ref(null);
  const authError = ref('');

  let unlistenSuccess = null;
  let unlistenError = null;

  onMounted(async () => {
    if (!window.__TAURI_INTERNALS__) return;
    const { listen } = await import('@tauri-apps/api/event');

    unlistenSuccess = await listen('auth-success', async () => {
      loggingInProvider.value = null;
      await refreshAccount();
      pushNotification(`Вход выполнен — ${accountStore.username}`);
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      await win.unminimize().catch(() => {});
      // Windows blocks a background process from stealing focus outright (e.g. from the
      // system browser during the ely.by flow) — toggling always-on-top forces it through.
      await win.setAlwaysOnTop(true).catch(() => {});
      await win.setFocus().catch(() => {});
      await win.setAlwaysOnTop(false).catch(() => {});
    });

    unlistenError = await listen('auth-error', (event) => {
      loggingInProvider.value = null;
      authError.value = String(event.payload);
    });
  });

  onUnmounted(() => {
    unlistenSuccess?.();
    unlistenError?.();
  });

  async function login(command, provider) {
    authError.value = '';
    loggingInProvider.value = provider;

    if (!window.__TAURI_INTERNALS__) {
      authError.value = 'Доступно только в приложении';
      loggingInProvider.value = null;
      return;
    }

    const { invoke } = await import('@tauri-apps/api/core');
    try {
      await invoke(command);
    } catch (err) {
      loggingInProvider.value = null;
      authError.value = String(err);
    }
  }

  const loginMicrosoft = () => login('login_microsoft', 'microsoft');
  const loginElyBy = () => login('login_elyby', 'elyby');

  async function cancelLogin() {
    if (loggingInProvider.value === 'elyby' && window.__TAURI_INTERNALS__) {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('cancel_elyby_login').catch(() => {});
    }
    loggingInProvider.value = null;
    authError.value = '';
  }

  async function logout() {
    if (!window.__TAURI_INTERNALS__) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('logout');
    await refreshAccount();
  }

  return { loggingInProvider, authError, loginMicrosoft, loginElyBy, cancelLogin, logout };
}
