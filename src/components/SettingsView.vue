<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { pushNotification } from '../store/notifications.js';
import { showConfirmDialog } from '../store/confirmDialog.js';
import ArmedResetButton from './ArmedResetButton.vue';

const scrollEl = ref(null);
const topFaded = ref(false);
const bottomFaded = ref(false);
function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  topFaded.value = el.scrollTop > 4;
  bottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 4;
}

const ram = ref(4);
const ramMax = ref(16);
const jvmArgs = ref('');
const onLaunch = ref('hide');
const autoConnect = ref(false);
const showLogs = ref(false);
const crashAnalyzer = ref(true);
const gameDir = ref(null);
const defaultGameDir = ref('');
const ramDragging = ref(false);

// Set directly, not tweened — animating toward each drag `input` event yanks the thumb back under the cursor mid-drag
function onRamInput(event) {
  ram.value = Number(event.target.value);
}

const movingFiles = ref(false);

async function pickGameDir() {
  if (!window.__TAURI_INTERNALS__) return;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ directory: true, title: 'Папка для файлов игры' });
  if (!picked) return;

  const fromDir = gameDir.value || defaultGameDir.value;
  const choice = await showConfirmDialog({
    title: 'Перенести файлы игры?',
    message: 'Перенести уже установленные файлы игры (версии, библиотеки, ресурсы) в новую папку? Если отказаться - при следующем запуске они скачаются заново',
    buttons: [
      { label: 'Да', value: 'move', variant: 'primary' },
      { label: 'Нет', value: 'skip', variant: 'ghost' },
      { label: 'Отмена', value: null, variant: 'ghost' },
    ],
  });
  if (choice === null) return;

  if (choice === 'move') {
    movingFiles.value = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('move_game_files', { from: fromDir, to: picked });
      pushNotification('Файлы игры перенесены');
    } catch (err) {
      pushNotification(`Не удалось перенести файлы: ${err}`, 'error');
    } finally {
      movingFiles.value = false;
    }
  }

  gameDir.value = picked;
}

function onResetGameDir() {
  gameDir.value = null;
  pushNotification('Папка с игрой сброшена');
}

const DEFAULT_JVM_ARGS = '-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions';

function onResetJvmArgs() {
  jvmArgs.value = DEFAULT_JVM_ARGS;
  pushNotification('Аргументы JVM сброшены');
}

const ON_LAUNCH_OPTIONS = [
  { value: 'none', label: 'Ничего не делать' },
  { value: 'hide', label: 'Скрыть лаунчер' },
  { value: 'close', label: 'Закрыть лаунчер' },
];
const onLaunchLabel = computed(() => ON_LAUNCH_OPTIONS.find((o) => o.value === onLaunch.value)?.label ?? '');
const onLaunchMenuOpen = ref(false);
const onLaunchWrapEl = ref(null);
const onLaunchTriggerEl = ref(null);

// FLIP: measure the width before/after so the trigger button animates instead of snapping
function selectOnLaunch(value) {
  const el = onLaunchTriggerEl.value;
  if (!el) {
    onLaunch.value = value;
    onLaunchMenuOpen.value = false;
    return;
  }
  const fromWidth = el.getBoundingClientRect().width;
  onLaunch.value = value;
  onLaunchMenuOpen.value = false;

  nextTick(() => {
    el.style.transition = 'none';
    el.style.width = 'auto';
    const toWidth = el.getBoundingClientRect().width;
    el.style.width = `${fromWidth}px`;
    void el.offsetWidth;
    el.style.transition = '';
    requestAnimationFrame(() => {
      el.style.width = `${toWidth}px`;
    });
    const clear = () => {
      el.style.width = '';
      el.style.transition = '';
      el.removeEventListener('transitionend', clear);
    };
    el.addEventListener('transitionend', clear);
  });
}

function handleOutsideClick(event) {
  if (onLaunchMenuOpen.value && onLaunchWrapEl.value && !onLaunchWrapEl.value.contains(event.target)) {
    onLaunchMenuOpen.value = false;
  }
}
onMounted(() => document.addEventListener('click', handleOutsideClick));
onUnmounted(() => document.removeEventListener('click', handleOutsideClick));

const settingsLoaded = ref(false);
const appVersion = ref('');

onMounted(async () => {
  if (!window.__TAURI_INTERNALS__) {
    settingsLoaded.value = true;
    nextTick(onScroll);
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core');
  const settings = await invoke('get_settings');
  ram.value = settings.ram_gb;
  jvmArgs.value = settings.jvm_args;
  onLaunch.value = settings.on_launch;
  autoConnect.value = settings.auto_connect;
  showLogs.value = settings.show_logs;
  crashAnalyzer.value = settings.crash_analyzer;
  gameDir.value = settings.game_dir;
  defaultGameDir.value = await invoke('get_default_game_dir');
  invoke('get_total_ram_gb').then((total) => {
    if (total) ramMax.value = total;
  });
  settingsLoaded.value = true;
  nextTick(onScroll);

  const { getVersion } = await import('@tauri-apps/api/app');
  appVersion.value = await getVersion();
});

let saveTimer = null;
watch([ram, jvmArgs, onLaunch, autoConnect, showLogs, crashAnalyzer, gameDir], () => {
  if (!settingsLoaded.value || !window.__TAURI_INTERNALS__) return;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('save_settings', {
      settings: {
        ram_gb: Number(ram.value),
        jvm_args: jvmArgs.value,
        on_launch: onLaunch.value,
        auto_connect: autoConnect.value,
        show_logs: showLogs.value,
        crash_analyzer: crashAnalyzer.value,
        game_dir: gameDir.value,
      },
    });
  }, 350);
});

onUnmounted(() => {
  clearTimeout(saveTimer);
});
</script>

<template>
  <div class="settings">
    <h1>Настройки</h1>

    <div class="settings-scroll-wrap">
    <div class="fade fade-top" :class="{ show: topFaded }"></div>

    <div class="settings-scroll" ref="scrollEl" @scroll="onScroll">
    <div class="settings-body">
    <Transition name="crossfade">
    <div v-if="!settingsLoaded" key="loading" class="settings-skeleton">
      <div v-for="i in 3" :key="i" class="group-skeleton">
        <span class="sk-line short"></span>
        <span class="sk-line"></span>
        <span class="sk-line"></span>
      </div>
    </div>

    <div v-else key="content" class="settings-groups">
    <section class="group">
      <h2>Ресурсы</h2>
      <div class="row">
        <div class="row-text">
          <span class="row-title">Выделено памяти</span>
          <span class="row-sub">Рекомендуем не больше половины от общего объёма ОЗУ</span>
        </div>
        <div class="ram-control">
          <input
            type="range"
            min="1"
            :max="ramMax"
            step="1"
            :value="ram"
            @input="onRamInput"
            :class="{ dragging: ramDragging }"
            @mousedown="ramDragging = true"
            @touchstart="ramDragging = true"
            @mouseup="ramDragging = false"
            @touchend="ramDragging = false"
            @blur="ramDragging = false"
          />
          <span class="ram-value mono" :class="{ dragging: ramDragging }">{{ ram }} ГБ</span>
        </div>
      </div>
      <div class="row row-col">
        <div class="row-text">
          <span class="row-title">Аргументы JVM</span>
          <span class="row-sub">Дополнительные флаги для запуска Java</span>
        </div>
        <div class="path-row">
          <input type="text" class="text-input mono" v-model="jvmArgs" spellcheck="false" />
          <ArmedResetButton title="Сбросить к значению по умолчанию" @confirm="onResetJvmArgs" />
        </div>
      </div>
      <div class="row row-col">
        <div class="row-text">
          <span class="row-title">Папка с игрой</span>
          <span class="row-sub">Куда устанавливаются файлы Minecraft (версии, библиотеки, моды, сохранения)</span>
        </div>
        <div class="path-row">
          <span class="path-value mono">{{ gameDir || defaultGameDir }}</span>
          <button class="browse-btn" :disabled="movingFiles" @click="pickGameDir">{{ movingFiles ? 'Переношу файлы...' : 'Обзор...' }}</button>
          <ArmedResetButton v-if="gameDir" title="Сбросить папку с игрой" :disabled="movingFiles" @confirm="onResetGameDir" />
        </div>
      </div>
    </section>

    <section class="group">
      <h2>Поведение лаунчера</h2>
      <div class="row">
        <div class="row-text">
          <span class="row-title">При запуске игры</span>
          <span class="row-sub">Что делать с окном лаунчера, пока вы в игре</span>
        </div>
        <div class="dropdown-wrap" ref="onLaunchWrapEl">
          <button class="dropdown-trigger" ref="onLaunchTriggerEl" @click="onLaunchMenuOpen = !onLaunchMenuOpen">
            {{ onLaunchLabel }}
            <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor" :class="{ open: onLaunchMenuOpen }"><path d="M7 10l5 5 5-5H7Z"/></svg>
          </button>
          <Transition name="dropdown">
            <div v-if="onLaunchMenuOpen" class="dropdown-menu">
              <button
                v-for="opt in ON_LAUNCH_OPTIONS"
                :key="opt.value"
                class="dropdown-item"
                :class="{ active: onLaunch === opt.value }"
                @click="selectOnLaunch(opt.value)"
              >
                {{ opt.label }}
              </button>
            </div>
          </Transition>
        </div>
      </div>
      <div class="row">
        <div class="row-text">
          <span class="row-title">Автоподключение к lemicraft.ru</span>
          <span class="row-sub">Сразу зайти на сервер при запуске игры</span>
        </div>
        <button class="toggle" :class="{ on: autoConnect }" @click="autoConnect = !autoConnect">
          <span class="knob"></span>
        </button>
      </div>
      <div class="row">
        <div class="row-text">
          <span class="row-title">Показывать окно с логами</span>
          <span class="row-sub">Открывать окно логов игры автоматически при запуске</span>
        </div>
        <button class="toggle" :class="{ on: showLogs }" @click="showLogs = !showLogs">
          <span class="knob"></span>
        </button>
      </div>
      <div class="row">
        <div class="row-text">
          <span class="row-title">Анализатор крашей</span>
          <span class="row-sub">Открывать окно логов, если игра завершилась аварийно</span>
        </div>
        <button class="toggle" :class="{ on: crashAnalyzer }" @click="crashAnalyzer = !crashAnalyzer">
          <span class="knob"></span>
        </button>
      </div>
    </section>

    <section class="group">
      <h2>О лаунчере</h2>
      <div class="row">
        <div class="row-text">
          <span class="row-title">Версия</span>
          <span class="row-sub">LemiCraft Launcher</span>
        </div>
        <span class="version mono">{{ appVersion || '…' }}</span>
      </div>
    </section>

    </div>
    </Transition>
    </div>
    </div>

    <div class="fade fade-bottom" :class="{ show: bottomFaded }"></div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  gap: 22px;
  height: 100%;
  min-height: 0;
}

.settings-scroll-wrap {
  position: relative;
  min-height: 0;
  flex: 1;
}

.settings-scroll {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
}

.fade {
  position: absolute;
  left: 0;
  right: 10px;
  height: 14px;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease;
  z-index: 1;
}
.fade.show {
  opacity: 1;
}
.fade-top {
  top: 0;
  background: linear-gradient(var(--bg), transparent);
}
.fade-bottom {
  bottom: 0;
  background: linear-gradient(transparent, var(--bg));
}

/* Not on .settings itself — App.vue's page-switch transition applies position: absolute to that */
.settings-body {
  position: relative;
}

.settings-groups,
.settings-skeleton {
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.crossfade-enter-active,
.crossfade-leave-active {
  transition: opacity 0.25s ease;
}
.crossfade-enter-from,
.crossfade-leave-to {
  opacity: 0;
}
.crossfade-leave-active {
  position: absolute;
  left: 0;
  right: 4px;
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

.group-skeleton {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-width: 640px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 18px 20px;
}

.sk-line {
  display: block;
  height: 13px;
  width: 100%;
  border-radius: 6px;
  background: var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}
.sk-line.short {
  width: 40%;
}

.group {
  animation: group-in 0.35s cubic-bezier(0.16, 1, 0.3, 1) backwards;
}
.group:nth-of-type(2) {
  animation-delay: 40ms;
}
.group:nth-of-type(3) {
  animation-delay: 80ms;
}

@keyframes group-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.settings h1 {
  font-size: 26px;
  font-weight: 800;
  color: var(--text);
}

.group {
  max-width: 640px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 8px 20px;
}

.group h2 {
  font-size: 12px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--text-faint);
  padding: 14px 0 6px;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 14px 0;
  border-top: 1px solid var(--border);
}
.group h2 + .row {
  border-top: none;
}

.row-text {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.row-col {
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
}

.text-input {
  width: 100%;
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
}
.text-input:focus {
  border-color: var(--accent);
}

.path-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.path-value {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.browse-btn {
  flex-shrink: 0;
  padding: 9px 14px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.browse-btn:hover:not(:disabled) {
  background: var(--surface-hover);
}
.browse-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.row-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}

.row-sub {
  font-size: 12.5px;
  color: var(--text-muted);
}

.ram-control {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.ram-control input[type='range'] {
  width: 140px;
  accent-color: var(--accent);
}
.ram-control input[type='range']::-webkit-slider-thumb {
  transition: transform 0.12s ease;
}
.ram-control input[type='range'].dragging::-webkit-slider-thumb,
.ram-control input[type='range']:active::-webkit-slider-thumb {
  transform: scale(1.25);
}

.ram-value {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  width: 48px;
  text-align: right;
  transition: color 0.15s ease, transform 0.12s ease;
}
.ram-value.dragging {
  color: var(--accent);
  transform: scale(1.08);
}

.dropdown-wrap {
  position: relative;
  flex-shrink: 0;
}

.dropdown-trigger {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 9px 14px;
  border-radius: 9px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  transition: background 0.15s ease, width 0.22s ease;
}
.dropdown-trigger:hover {
  background: var(--surface-hover);
}
.dropdown-trigger svg {
  color: var(--text-faint);
  transition: transform 0.15s ease;
}
.dropdown-trigger svg.open {
  transform: rotate(180deg);
}

.dropdown-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 10;
  min-width: 180px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-pop);
}

.dropdown-item {
  padding: 8px 10px;
  border-radius: 7px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 600;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.dropdown-item:hover {
  background: var(--surface-2);
  color: var(--text);
}
.dropdown-item.active {
  color: var(--accent);
}

.dropdown-enter-active,
.dropdown-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.toggle {
  width: 40px;
  height: 24px;
  border-radius: 999px;
  border: none;
  background: var(--surface-2);
  padding: 3px;
  cursor: pointer;
  flex-shrink: 0;
  transition: background 0.15s ease;
}
.toggle.on {
  background: var(--accent);
}

.knob {
  display: block;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.15s ease;
}
.toggle.on .knob {
  transform: translateX(16px);
}

.version {
  font-size: 13px;
  color: var(--text-muted);
}

@media (prefers-reduced-motion: reduce) {
  .group {
    animation: none;
  }
  .sk-line {
    animation: none;
  }
  .crossfade-enter-active,
  .crossfade-leave-active {
    transition: none;
  }
}
</style>
