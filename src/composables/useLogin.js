import { ref } from 'vue';
import { applyAccount, refreshAccount } from '../store/account.js';
import { pushNotification } from '../store/notifications.js';

// Module-level so the sidebar and the full-screen login overlay see the same login
const loggingInProvider = ref(null);
const authError = ref('');

const LOGIN_ERROR_MS = 6000;
const REFRESH_WAIT_MS = 3000;

let listening = false;

async function ensureListeners() {
  if (listening || !window.__TAURI_INTERNALS__) return;
  listening = true;
  const { listen } = await import('@tauri-apps/api/event');

  await listen('auth-success', async (event) => {
    // From the event, not get_current_account: that re-validates the token and a hiccup would wipe the fresh login.
    // The overlay waits for it (capped, the skin download can be slow) so "Гость" never flashes after it closes
    try {
      await Promise.race([applyAccount(event.payload), new Promise((resolve) => setTimeout(resolve, REFRESH_WAIT_MS))]);
    } finally {
      loggingInProvider.value = null;
    }
    pushNotification(`Вход выполнен — ${event.payload.username}`);
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    await win.unminimize().catch(() => {});
    // Windows won't hand focus to a background process; toggling always-on-top forces it
    await win.setAlwaysOnTop(true).catch(() => {});
    await win.setFocus().catch(() => {});
    await win.setAlwaysOnTop(false).catch(() => {});
  });

  await listen('auth-error', (event) => {
    // A cancelled flow can still report an error afterwards
    if (!loggingInProvider.value) return;
    loggingInProvider.value = null;
    reportError(String(event.payload));
  });
}

// The overlay closes on failure and the account menu is already shut, so the error needs its own toast
function reportError(message) {
  authError.value = message;
  pushNotification(message, 'error', LOGIN_ERROR_MS);
}

async function login(command, provider) {
  authError.value = '';
  loggingInProvider.value = provider;

  if (!window.__TAURI_INTERNALS__) {
    loggingInProvider.value = null;
    reportError('Доступно только в приложении');
    return;
  }

  const { invoke } = await import('@tauri-apps/api/core');
  try {
    await invoke(command);
  } catch (err) {
    loggingInProvider.value = null;
    reportError(String(err));
  }
}

const loginMicrosoft = () => login('login_microsoft', 'microsoft');
const loginElyBy = () => login('login_elyby', 'elyby');

async function cancelLogin() {
  const provider = loggingInProvider.value;
  loggingInProvider.value = null;
  authError.value = '';
  if (!provider || !window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke(provider === 'elyby' ? 'cancel_elyby_login' : 'cancel_microsoft_login').catch(() => {});
}

async function logout() {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('logout');
  await refreshAccount();
}

export function useLogin() {
  ensureListeners();
  return { loggingInProvider, authError, loginMicrosoft, loginElyBy, cancelLogin, logout };
}
