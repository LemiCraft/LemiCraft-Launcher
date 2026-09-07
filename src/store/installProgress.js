import { reactive } from 'vue';
import { accountStore } from './account.js';
import { pushNotification } from './notifications.js';
import { modsState } from './mods.js';
import { showProgress, showProgressError, hideProgress } from './progressToast.js';

export const installState = reactive({
  isInstalling: false,
  isGameRunning: false,
  isInstalled: null,
});

let started = false;

export async function refreshInstalledState() {
  if (!window.__TAURI_INTERNALS__) return;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    installState.isInstalled = await invoke('is_game_installed');
  } catch {}
}

const stageLabels = {
  version: 'Загрузка версии Minecraft...',
  libraries: 'Загрузка библиотек...',
  assets: 'Загрузка ресурсов...',
  jvm: 'Проверка Java...',
  authlib: 'Настройка входа через ely.by...',
  launched: 'Запущено!',
};

export async function startInstallListeners() {
  if (started || !window.__TAURI_INTERNALS__) return;
  started = true;
  const { listen } = await import('@tauri-apps/api/event');

  await listen('install-progress', (event) => {
    const p = event.payload;
    if (p.stage === 'download') {
      showProgress(`Скачивание файлов: ${p.count}/${p.totalCount}`, p.totalCount > 0 ? Math.round((p.count / p.totalCount) * 100) : 0);
    } else if (p.stage === 'retry') {
      showProgress(`Часть файлов не скачалась, повтор (${p.attempt}/${p.maxAttempts})...`);
    } else if (p.stage === 'launched') {
      showProgress(stageLabels.launched, 100);
      installState.isInstalled = true;
      setTimeout(() => {
        installState.isInstalling = false;
        hideProgress();
      }, 1500);
    } else {
      showProgress(stageLabels[p.stage] || '');
    }
  });

  await listen('install-error', (event) => {
    installState.isInstalling = false;
    showProgressError(typeof event.payload === 'string' ? event.payload : 'Не удалось запустить игру');
  });

  await listen('game-started', async () => {
    installState.isGameRunning = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const settings = await invoke('get_settings');
      if (settings.show_logs) await invoke('open_logs_window');
    } catch {}
  });

  await listen('game-stopped', () => {
    installState.isGameRunning = false;
  });

  await listen('game-crashed', async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const settings = await invoke('get_settings');
      if (settings.crash_analyzer) {
        pushNotification('Игра завершилась аварийно — открываю логи', 'warn');
        await invoke('open_logs_window');
      }
    } catch {}
  });
}

export async function launchGame() {
  if (installState.isInstalling || installState.isGameRunning || modsState.applying) return;
  if (!accountStore.loggedIn) {
    showProgressError('Сначала войдите в аккаунт');
    return;
  }
  showProgress('Запуск...');
  installState.isInstalling = true;

  if (!window.__TAURI_INTERNALS__) {
    showProgress('Доступно только в приложении');
    setTimeout(() => {
      installState.isInstalling = false;
      hideProgress();
    }, 2000);
    return;
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('play', { username: accountStore.username });
  } catch (err) {
    installState.isInstalling = false;
    showProgressError(String(err));
  }
}
