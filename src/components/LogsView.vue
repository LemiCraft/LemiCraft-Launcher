<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue';

const lines = ref([]);
const autoScroll = ref(true);
const scrollEl = ref(null);
const MAX_LINES = 3000;
const copyState = ref('idle');

const isRunning = ref(false);
const elapsedSeconds = ref(0);
let tickTimer = null;

let unlistenLog = null;
let unlistenStarted = null;
let unlistenStopped = null;

function classifyLine(line) {
  if (/\bERROR\b|\bFATAL\b|Exception/i.test(line)) return 'err';
  if (/\bWARN\b/i.test(line)) return 'warn';
  if (/\bDEBUG\b/i.test(line)) return 'debug';
  return '';
}

function pad(n) {
  return String(n).padStart(2, '0');
}
const formattedElapsed = computed(() => {
  const total = elapsedSeconds.value;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return `${pad(h)}:${pad(m)}:${pad(s)}`;
});

function handleGameStarted() {
  isRunning.value = true;
  elapsedSeconds.value = 0;
  clearInterval(tickTimer);
  tickTimer = setInterval(() => {
    elapsedSeconds.value++;
  }, 1000);
}

function handleGameStopped() {
  isRunning.value = false;
  clearInterval(tickTimer);
  tickTimer = null;
}

async function stopGame() {
  if (!window.__TAURI_INTERNALS__ || !isRunning.value) return;
  const { invoke } = await import('@tauri-apps/api/core');
  try {
    await invoke('stop_game');
  } catch (err) {
    console.error('failed to stop game:', err);
  }
}

onMounted(async () => {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');

  // The window may open mid-session (game already running) — catch up on what stdout
  // piping alone would've missed by reading Minecraft's own log file once up front.
  try {
    if (await invoke('is_game_running')) {
      handleGameStarted();
      const history = await invoke('read_current_log');
      const historyLines = history.split('\n').filter((text) => text.length > 0);
      lines.value = historyLines.map((text) => ({ text, level: classifyLine(text) }));
      if (lines.value.length > MAX_LINES) lines.value.splice(0, lines.value.length - MAX_LINES);
      nextTick(() => {
        if (scrollEl.value) scrollEl.value.scrollTop = scrollEl.value.scrollHeight;
      });
    }
  } catch {}

  unlistenLog = await listen('game-log', (event) => {
    const text = event.payload;
    lines.value.push({ text, level: classifyLine(text) });
    if (lines.value.length > MAX_LINES) {
      lines.value.splice(0, lines.value.length - MAX_LINES);
    }
    if (autoScroll.value) {
      nextTick(() => {
        if (scrollEl.value) scrollEl.value.scrollTop = scrollEl.value.scrollHeight;
      });
    }
  });

  unlistenStarted = await listen('game-started', handleGameStarted);
  unlistenStopped = await listen('game-stopped', handleGameStopped);
});

onUnmounted(() => {
  unlistenLog?.();
  unlistenStarted?.();
  unlistenStopped?.();
  clearInterval(tickTimer);
});

function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
}

function clearLogs() {
  lines.value = [];
}

function scrollToBottom() {
  autoScroll.value = true;
  if (scrollEl.value) scrollEl.value.scrollTop = scrollEl.value.scrollHeight;
}

async function copyLogs() {
  try {
    await navigator.clipboard.writeText(lines.value.map((l) => l.text).join('\n'));
    copyState.value = 'success';
  } catch {
    copyState.value = 'error';
  }
  setTimeout(() => (copyState.value = 'idle'), 1500);
}
</script>

<template>
  <div class="logs">
    <div class="head">
      <div class="status-row">
        <span class="status-dot" :class="{ running: isRunning }"></span>
        <span class="status-text">{{ isRunning ? 'Игра запущена' : 'Игра не запущена' }}</span>
        <span v-if="isRunning" class="timer mono">{{ formattedElapsed }}</span>
      </div>
      <div class="head-actions">
        <button class="upload-btn" :disabled="lines.length === 0" @click="copyLogs">
          {{ copyState === 'success' ? 'Скопировано' : copyState === 'error' ? 'Не удалось' : 'Копировать' }}
        </button>
        <button class="upload-btn danger" :disabled="!isRunning" @click="stopGame">Стоп</button>
        <button class="upload-btn" @click="clearLogs">Очистить</button>
      </div>
    </div>

    <div class="console-wrap">
      <div class="console" ref="scrollEl" @scroll="onScroll">
        <p v-if="lines.length === 0" class="empty">Пока пусто — запустите игру, чтобы увидеть лог здесь</p>
        <p v-for="(line, i) in lines" :key="i" class="line" :class="line.level">{{ line.text }}</p>
      </div>
      <button v-if="!autoScroll && lines.length > 0" class="scroll-bottom-btn" @click="scrollToBottom">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 16 5 9l1.4-1.4L12 13.2l5.6-5.6L19 9Z"/></svg>
        К низу
      </button>
    </div>
  </div>
</template>

<style scoped>
.logs {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  min-height: 0;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.status-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-faint);
  flex-shrink: 0;
}
.status-dot.running {
  background: var(--good);
  box-shadow: 0 0 0 3px var(--good-soft);
}

.status-text {
  color: var(--text-muted);
  font-size: 13.5px;
}

.timer {
  color: var(--text-faint);
  font-size: 12.5px;
}

.head-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.upload-btn {
  flex-shrink: 0;
  padding: 9px 16px;
  border-radius: 9px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.upload-btn:hover {
  background: var(--surface-hover);
}
.upload-btn.danger {
  color: var(--bad);
}
.upload-btn.danger:hover:not(:disabled) {
  background: var(--bad-soft);
}
.upload-btn:disabled {
  opacity: 0.45;
  cursor: default;
}

.console-wrap {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
}

.console {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  background: #0c0b0e;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 14px 16px;
}

.scroll-bottom-btn {
  position: absolute;
  bottom: 14px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-pop);
  transition: background 0.15s ease;
}
.scroll-bottom-btn:hover {
  background: var(--surface-hover);
}

.empty {
  color: var(--text-faint);
  font-size: 13px;
  font-family: var(--font-mono);
}

.line {
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
  color: #cfcbd4;
  white-space: pre-wrap;
  word-break: break-word;
}
.line.warn {
  color: #e0b25a;
}
.line.err {
  color: #e07a6f;
}
.line.debug {
  color: #7d7885;
}
</style>
