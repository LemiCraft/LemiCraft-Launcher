<script setup>
import { onMounted, ref, watch } from 'vue';
import WindowControls from './components/WindowControls.vue';
import AppSidebar from './components/AppSidebar.vue';
import HomeView from './components/HomeView.vue';
import SkinsView from './components/SkinsView.vue';
import ModsView from './components/ModsView.vue';
import LogsView from './components/LogsView.vue';
import SettingsView from './components/SettingsView.vue';
import RulesView from './components/RulesView.vue';
import ProgressToast from './components/ProgressToast.vue';
import NotificationStack from './components/NotificationStack.vue';
import UpdateModal from './components/UpdateModal.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import { refreshAccount } from './store/account.js';
import { startInstallListeners } from './store/installProgress.js';
import { skinsState, startElybyLoginListeners } from './store/skins.js';
import { checkForUpdate } from './store/update.js';
import { modsState, startModsProgressListener, startDeepLinkListener } from './store/mods.js';
import { startViolationsWatcher } from './store/violations.js';

const isLogsWindow = ref(false);
const windowResolved = ref(!window.__TAURI_INTERNALS__);

const active = ref('home');
const isMaximized = ref(false);
const isClosing = ref(false);

// A lemicraft://import/<code> link switches to the mods page; ModsView picks up pendingImportCode itself
watch(
  () => modsState.pendingImportCode,
  (code) => {
    if (code) active.value = 'mods';
  },
);

onMounted(async () => {
  if (window.__TAURI_INTERNALS__) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    isLogsWindow.value = getCurrentWindow().label === 'logs';
    windowResolved.value = true;
  }
  if (!isLogsWindow.value) {
    refreshAccount();
    startInstallListeners();
    startElybyLoginListeners();
    checkForUpdate();
    startModsProgressListener();
    startDeepLinkListener();
    startViolationsWatcher();
  }
});
</script>

<template>
  <template v-if="windowResolved">
    <div v-if="isLogsWindow" class="shell logs-shell" :class="{ closing: isClosing }">
      <WindowControls title="Логи игры" :fade-on-close="false" @update:maximized="isMaximized = $event" @closing="isClosing = true" />
      <main class="content logs-content">
        <LogsView />
      </main>
    </div>

    <div v-else class="shell" :class="{ maximized: isMaximized, closing: isClosing }">
      <AppSidebar :active="active" @navigate="active = $event" />
      <div class="content-col">
        <WindowControls guard-mods @update:maximized="isMaximized = $event" @closing="isClosing = true" />
        <main class="content">
          <Transition name="view">
            <HomeView v-if="active === 'home'" key="home" @navigate="active = $event" />
            <SkinsView v-else-if="active === 'skins'" key="skins" />
            <ModsView v-else-if="active === 'mods'" key="mods" />
            <RulesView v-else-if="active === 'rules'" key="rules" />
            <SettingsView v-else-if="active === 'settings'" key="settings" />
          </Transition>
        </main>
      </div>
      <ProgressToast />
      <NotificationStack />
      <UpdateModal />
      <ConfirmDialog />
      <Transition name="dim">
        <div v-if="skinsState.elybyLoginPending" class="login-dim"></div>
      </Transition>
    </div>
  </template>
</template>

<style scoped>
.shell {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  box-shadow: 0 0 0 1px var(--border), 0 30px 80px -20px rgba(0, 0, 0, 0.7);
  animation: shell-in 0.35s ease;
}

.shell:not(.logs-shell) {
  flex-direction: row;
}

.shell.logs-shell,
.shell.logs-shell.closing {
  animation: none;
}

.shell.maximized {
  box-shadow: none;
}

.shell.closing {
  animation: shell-out 0.18s ease forwards;
}
@keyframes shell-out {
  from {
    opacity: 1;
  }
  to {
    opacity: 0;
  }
}

.content-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.content {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  padding: 8px 36px 28px;
  display: flex;
  flex-direction: column;
}

.logs-content {
  padding: 12px 16px 16px;
}

.content > * {
  height: 100%;
  min-height: 0;
}

@keyframes shell-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.view-enter-active,
.view-leave-active {
  transition: opacity 0.18s ease;
}
.view-leave-active {
  position: absolute;
  inset: 8px 36px 28px 36px;
  height: auto;
  z-index: 1;
}
.view-enter-from,
.view-leave-to {
  opacity: 0;
}

.login-dim {
  position: absolute;
  inset: 0;
  background: rgba(8, 7, 9, 0.55);
  z-index: 50;
  cursor: default;
}

.dim-enter-active,
.dim-leave-active {
  transition: opacity 0.2s ease;
}
.dim-enter-from,
.dim-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .shell,
  .shell.closing {
    animation: none;
  }
  .view-enter-active,
  .view-leave-active,
  .dim-enter-active,
  .dim-leave-active {
    transition: none;
  }
}
</style>
