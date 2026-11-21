<script setup>
import { reactive } from 'vue';
import { notifications, dismissNotification } from '../store/notifications.js';
import { copyText } from '../utils/clipboard.js';

// the id of a notice whose text was just copied: its button shows a tick for a moment
const copied = reactive({});
async function copy(n) {
  if (!(await copyText(n.text))) return;
  copied[n.id] = true;
  setTimeout(() => delete copied[n.id], 1500);
}
</script>

<template>
  <div class="notif-stack">
    <TransitionGroup name="notif">
      <div v-for="n in notifications" :key="n.id" class="notif" :class="n.kind" @click="dismissNotification(n.id)">
        <svg v-if="n.kind === 'success'" viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z"/></svg>
        <svg v-else-if="n.kind === 'error'" viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M12 2 1 21h22L12 2Zm0 6 1 7h-2l1-7Zm0 9.5a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5Z"/></svg>
        <svg v-else-if="n.kind === 'warn'" viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M12 2 1 21h22L12 2Zm0 6 1 7h-2l1-7Zm0 9.5a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5Z"/></svg>
        <span class="notif-text">{{ n.text }}</span>
        <button v-if="n.kind !== 'success'" type="button" class="notif-copy" :title="copied[n.id] ? 'Скопировано' : 'Скопировать текст'" :aria-label="copied[n.id] ? 'Текст скопирован' : 'Скопировать текст'" @click.stop="copy(n)">
          <svg v-if="copied[n.id]" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m5 12.5 4.5 4.5L19 7.5" /></svg>
          <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg>
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.notif-stack {
  position: fixed;
  top: 52px;
  right: 20px;
  z-index: 300;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
}

.notif {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 11px 15px;
  border-radius: 10px;
  background: var(--surface);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-pop);
  font-size: 13px;
  color: var(--text);
  cursor: pointer;
  max-width: 320px;
}
.notif.success {
  border-color: rgba(34, 197, 94, 0.35);
}
.notif.success svg {
  flex-shrink: 0;
  color: var(--good);
}
.notif.error {
  border-color: rgba(239, 68, 68, 0.35);
}
.notif.error svg {
  flex-shrink: 0;
  color: var(--bad);
}
.notif.warn {
  border-color: rgba(231, 200, 115, 0.35);
}
.notif.warn svg {
  flex-shrink: 0;
  color: var(--gold);
}

.notif-text {
  overflow-wrap: anywhere;
}
.notif .notif-copy {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  margin: -4px -6px -4px 0;
  border: none;
  border-radius: 8px;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.notif .notif-copy:hover {
  background: var(--surface-hover);
  color: var(--text);
}
.notif.error .notif-copy svg,
.notif.warn .notif-copy svg {
  color: inherit;
}

.notif-enter-active,
.notif-leave-active {
  transition: opacity 0.2s ease, transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}
.notif-enter-from,
.notif-leave-to {
  opacity: 0;
  transform: translateX(16px);
}
.notif-leave-active {
  position: absolute;
  right: 0;
  /* Explicit width: .notif-stack collapses toward zero once this is the last child */
  width: max-content;
  max-width: 320px;
}
.notif-move {
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

@media (prefers-reduced-motion: reduce) {
  .notif-enter-active,
  .notif-leave-active,
  .notif-move {
    transition: none;
  }
}
</style>
