import { reactive, watch } from 'vue';
import { accountStore } from './account.js';

// whether Mojang or Ely.by (the current account's) answers
export const providerStatus = reactive({
  checked: false,
  ok: true,
  provider: '',
  problems: [],
  checking: false,
});

let started = false;

function apply(status) {
  providerStatus.provider = status.provider;
  providerStatus.ok = status.ok;
  providerStatus.problems = status.problems;
  providerStatus.checked = true;
}

export async function checkProviderStatus() {
  if (!window.__TAURI_INTERNALS__ || !accountStore.loggedIn) {
    apply({ provider: '', ok: true, problems: [] });
    return;
  }
  providerStatus.checking = true;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    apply(await invoke('check_provider_status'));
  } catch (err) {
    console.error('provider status check failed:', err);
  } finally {
    providerStatus.checking = false;
  }
}

export async function startProviderStatusWatcher() {
  if (started) return;
  started = true;
  if (window.__TAURI_INTERNALS__) {
    // the backend also checks right before a game starts
    const { listen } = await import('@tauri-apps/api/event');
    await listen('provider-status', (event) => apply(event.payload));
  }
  watch(() => [accountStore.loggedIn, accountStore.provider], () => checkProviderStatus(), { immediate: true });
  setInterval(checkProviderStatus, 120000);
}
