<script setup>
import { computed, onMounted, onUnmounted } from 'vue';
import { useLogin } from '../composables/useLogin.js';

const { loggingInProvider, cancelLogin } = useLogin();

const hint = computed(() =>
  loggingInProvider.value === 'elyby'
    ? 'Ely.by — завершите вход в открывшемся браузере'
    : 'Microsoft — завершите вход в открывшемся окне',
);

function onKeydown(event) {
  if (event.key === 'Escape' && loggingInProvider.value) cancelLogin();
}

onMounted(() => window.addEventListener('keydown', onKeydown));
onUnmounted(() => window.removeEventListener('keydown', onKeydown));
</script>

<template>
  <Transition name="login-overlay">
    <div v-if="loggingInProvider" class="login-overlay">
      <div class="drag-strip"></div>
      <div class="login-card" role="alertdialog" aria-live="polite">
        <div class="spinner"></div>
        <h2>Вход...</h2>
        <p>{{ hint }}</p>
        <button class="cancel-btn" @click="cancelLogin">Отмена</button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.login-overlay {
  position: absolute;
  inset: 0;
  z-index: 120;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(8, 7, 9, 0.62);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
}

/* Keeps the window draggable by its title bar while the overlay covers it */
.drag-strip {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 40px;
  -webkit-app-region: drag;
}

.login-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  width: max-content;
  min-width: 300px;
  max-width: 90vw;
  padding: 28px 32px 22px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
  text-align: center;
}

.login-card h2 {
  font-size: 18px;
  font-weight: 800;
  color: var(--text);
}

.login-card p {
  font-size: 13px;
  white-space: nowrap;
  color: var(--text-muted);
}

.spinner {
  width: 34px;
  height: 34px;
  margin-bottom: 4px;
  border-radius: 50%;
  border: 3px solid var(--border);
  border-top-color: var(--accent);
  animation: login-spin 0.8s linear infinite;
}

@keyframes login-spin {
  to {
    transform: rotate(360deg);
  }
}

.cancel-btn {
  margin-top: 8px;
  padding: 9px 22px;
  border-radius: 9px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.cancel-btn:hover {
  background: var(--surface-hover);
}

.login-overlay-enter-active,
.login-overlay-leave-active {
  transition: opacity 0.2s ease;
}
.login-overlay-enter-from,
.login-overlay-leave-to {
  opacity: 0;
}
</style>
