<script setup>
import { onMounted, onUnmounted } from 'vue';
import { confirmDialogState, resolveConfirmDialog } from '../store/confirmDialog.js';

function onKeydown(event) {
  if (event.key === 'Escape' && confirmDialogState.open) resolveConfirmDialog(null);
}
onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));
</script>

<template>
  <Transition name="modal">
    <div v-if="confirmDialogState.open" class="overlay" @click.self="resolveConfirmDialog(null)">
      <div class="confirm-modal">
        <button class="close-btn" @click="resolveConfirmDialog(null)">
          <svg viewBox="0 0 10 10" width="11" height="11"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3"/></svg>
        </button>
        <h2>{{ confirmDialogState.title }}</h2>
        <p class="message">{{ confirmDialogState.message }}</p>
        <div class="actions">
          <button
            v-for="btn in confirmDialogState.buttons"
            :key="btn.value ?? 'null'"
            :class="['action-btn', btn.variant || 'ghost']"
            @click="resolveConfirmDialog(btn.value)"
          >
            {{ btn.label }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(8, 7, 9, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.confirm-modal {
  position: relative;
  width: min(440px, 88vw);
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 22px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
}

.confirm-modal h2 {
  font-size: 17px;
  font-weight: 800;
  color: var(--text);
  padding-right: 24px;
}

.message {
  color: var(--text-muted);
  font-size: 13.5px;
  line-height: 1.5;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 6px;
}

.action-btn {
  padding: 9px 16px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.action-btn.ghost {
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-muted);
}
.action-btn.ghost:hover {
  background: var(--surface-2);
}

.action-btn.primary {
  border: none;
  background: var(--accent);
  color: var(--accent-text-on);
  font-weight: 700;
}

.action-btn.danger {
  border: none;
  background: var(--bad);
  color: #fff;
  font-weight: 700;
}

.close-btn {
  position: absolute;
  top: 14px;
  right: 14px;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  border: none;
  background: transparent;
  color: var(--text-faint);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.close-btn:hover {
  background: var(--surface-2);
  color: var(--text);
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.15s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
</style>
