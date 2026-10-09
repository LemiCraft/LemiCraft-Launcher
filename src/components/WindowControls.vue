<script setup>
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { modsState } from '../store/mods.js';
import { showConfirmDialog } from '../store/confirmDialog.js';

const props = defineProps({
  title: { type: String, default: '' },
  fadeOnClose: { type: Boolean, default: true },
  guardMods: { type: Boolean, default: false },
});
const emit = defineEmits(['update:maximized', 'closing']);

const isMaximized = ref(false);
watch(isMaximized, (value) => emit('update:maximized', value), { immediate: true });
let unsubscribe = null;
let unsubscribeClose = null;

async function confirmCloseWhileApplying() {
  return showConfirmDialog({
    title: 'Установка модов не завершена',
    message: 'Если закрыть лаунчер сейчас, установка прервётся и часть файлов может остаться в неполном состоянии',
    buttons: [
      { label: 'Закрыть', value: true, variant: 'danger' },
      { label: 'Подождать', value: false, variant: 'ghost' },
    ],
  });
}

const isTauri = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
const isMac = typeof navigator !== 'undefined' && (/Macintosh|Mac OS X|MacPPC|MacIntel/i.test(navigator.userAgent || '') || navigator.platform?.toUpperCase().indexOf('MAC') >= 0);

let tauriWindow = null;
async function getTauriWindow() {
  if (!tauriWindow) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    tauriWindow = getCurrentWindow();
  }
  return tauriWindow;
}

async function minimize() {
  if (isTauri) (await getTauriWindow()).minimize();
}
async function toggleMaximize() {
  if (isTauri) (await getTauriWindow()).toggleMaximize();
}
const FADE_OUT_MS = 180;

async function close() {
  if (!isTauri) return;
  if (props.guardMods && modsState.applying && !(await confirmCloseWhileApplying())) return;
  const win = await getTauriWindow();
  const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  if (!props.fadeOnClose || reducedMotion) {
    win.destroy();
    return;
  }
  emit('closing');
  setTimeout(() => win.destroy(), FADE_OUT_MS);
}

onMounted(async () => {
  if (!isTauri) return;
  const win = await getTauriWindow();
  isMaximized.value = await win.isMaximized();
  unsubscribe = await win.onResized(async () => {
    isMaximized.value = await win.isMaximized();
  });
  // Safety net for close paths that skip the button above (Alt+F4, taskbar) — win.close()/destroy()
  // from the button also flows through here, but destroy() bypasses it, so no double prompt
  if (props.guardMods) {
    unsubscribeClose = await win.onCloseRequested(async (event) => {
      if (!modsState.applying) return;
      event.preventDefault();
      if (await confirmCloseWhileApplying()) win.destroy();
    });
  }
});
onUnmounted(() => {
  if (typeof unsubscribe === 'function') unsubscribe();
  if (typeof unsubscribeClose === 'function') unsubscribeClose();
});
</script>

<template>
  <div class="window-controls-bar" :class="{ 'is-mac': isMac, 'has-sidebar': guardMods }" @dblclick="toggleMaximize">
    <!-- Standalone windows on macOS (e.g. Logs window): traffic lights on left -->
    <div v-if="isMac && !guardMods" class="mac-controls-left">
      <button class="mac-btn mac-close" title="Закрыть" @click="close">
        <svg viewBox="0 0 10 10" width="6" height="6"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>
      </button>
      <button class="mac-btn mac-minimize" title="Свернуть" @click="minimize">
        <svg viewBox="0 0 10 10" width="6" height="6"><path d="M1 5h8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>
      </button>
      <button class="mac-btn mac-maximize" :title="isMaximized ? 'Восстановить' : 'Развернуть'" @click="toggleMaximize">
        <svg viewBox="0 0 10 10" width="6" height="6"><path d="M2 8l6-6M8 8V2H2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </button>
    </div>

    <div class="drag-fill" data-tauri-drag-region>
      <span v-if="title" class="bar-title">{{ title }}</span>
    </div>

    <!-- Windows / Linux controls on the right -->
    <div v-if="!isMac" class="controls">
      <button class="ctrl" title="Свернуть" @click="minimize">
        <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0 5h10" stroke="currentColor" stroke-width="1" /></svg>
      </button>
      <button class="ctrl" :title="isMaximized ? 'Восстановить' : 'Развернуть'" @click="toggleMaximize">
        <svg v-if="!isMaximized" viewBox="0 0 10 10" width="10" height="10">
          <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
        <svg v-else viewBox="0 0 10 10" width="10" height="10">
          <rect x="0.5" y="2" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
          <path d="M2.5 2V0.5H9.5V7.5H8" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button class="ctrl ctrl-close" title="Закрыть" @click="close">
        <svg viewBox="0 0 10 10" width="10" height="10">
          <path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.1" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.window-controls-bar {
  height: 40px;
  flex-shrink: 0;
  display: flex;
  align-items: stretch;
  user-select: none;
}

.drag-fill {
  flex: 1;
  -webkit-app-region: drag;
  display: flex;
  align-items: center;
  padding-left: 16px;
}
.is-mac .drag-fill {
  padding-left: 8px;
}

.bar-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-faint);
}

/* Above the login overlay (z-index 120), so the window can still be minimized or closed mid-login */
.controls {
  position: relative;
  z-index: 130;
  -webkit-app-region: no-drag;
  display: flex;
  height: 100%;
}

.ctrl {
  width: 44px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--text-faint);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}
.ctrl:hover {
  background: var(--surface-hover);
  color: var(--text);
}
.ctrl-close:hover {
  background: var(--bad);
  color: #fff;
}

.mac-controls-left {
  position: relative;
  z-index: 130;
  -webkit-app-region: no-drag;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
  flex-shrink: 0;
  height: 100%;
}

.mac-btn {
  width: 12px;
  height: 12px;
  min-width: 12px;
  min-height: 12px;
  border-radius: 50%;
  border: 0.5px solid rgba(0, 0, 0, 0.25);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: 0;
  color: transparent;
  box-shadow: inset 0 1px 1px rgba(255, 255, 255, 0.15);
  flex-shrink: 0;
  aspect-ratio: 1 / 1;
  transition: transform 0.1s ease, filter 0.12s ease;
}

.mac-controls-left:hover .mac-btn {
  color: rgba(0, 0, 0, 0.68);
}

.mac-close {
  background: #ff5f56;
  border-color: #e0443e;
}
.mac-close:hover {
  background: #ff6961;
}
.mac-close:active {
  background: #bf4942;
}

.mac-minimize {
  background: #ffbd2e;
  border-color: #dea123;
}
.mac-minimize:hover {
  background: #ffc73d;
}
.mac-minimize:active {
  background: #bf8e22;
}

.mac-maximize {
  background: #27c93f;
  border-color: #1aab29;
}
.mac-maximize:hover {
  background: #34d64c;
}
.mac-maximize:active {
  background: #1d9630;
}

</style>
