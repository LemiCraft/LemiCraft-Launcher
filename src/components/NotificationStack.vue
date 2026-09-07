<script setup>
import { notifications, dismissNotification } from '../store/notifications.js';
</script>

<template>
  <div class="notif-stack">
    <TransitionGroup name="notif">
      <div v-for="n in notifications" :key="n.id" class="notif" :class="n.kind" @click="dismissNotification(n.id)">
        <svg v-if="n.kind === 'success'" viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z"/></svg>
        <svg v-else-if="n.kind === 'error'" viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M12 2 1 21h22L12 2Zm0 6 1 7h-2l1-7Zm0 9.5a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5Z"/></svg>
        <svg v-else-if="n.kind === 'warn'" viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M12 2 1 21h22L12 2Zm0 6 1 7h-2l1-7Zm0 9.5a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5Z"/></svg>
        <span class="notif-text">{{ n.text }}</span>
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
