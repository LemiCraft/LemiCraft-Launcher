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
  <div class="window-controls-bar" @dblclick="toggleMaximize">
    <div class="drag-fill" data-tauri-drag-region>
      <span v-if="title" class="bar-title">{{ title }}</span>
    </div>
    <div class="controls">
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

.bar-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-faint);
}

.controls {
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
</style>
