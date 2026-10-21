<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { providerStatus, checkProviderStatus } from '../store/providerStatus.js';

const open = ref(false);
const isElyBy = computed(() => providerStatus.provider === 'Ely.by');
const name = computed(() => (isElyBy.value ? 'Ely.by' : 'Mojang'));
const hint = computed(() =>
  isElyBy.value
    ? 'Запуск игры с этим аккаунтом, скорее всего, не получится: попробуйте позже'
    : 'Вход, обновление аккаунта и вход на серверы могут не работать: попробуйте позже',
);

watch(() => providerStatus.ok, (ok) => {
  if (ok) open.value = false;
});

const close = () => { open.value = false; };
onMounted(() => window.addEventListener('click', close));
onBeforeUnmount(() => window.removeEventListener('click', close));
</script>

<template>
  <!-- A corner badge on the hero card: it overlays the card instead of taking room in it -->
  <Transition name="badge">
    <div v-if="providerStatus.checked && !providerStatus.ok" class="provider-badge" @click.stop>
      <button class="pill" type="button" :aria-expanded="open" @click="open = !open">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true">
          <path d="M12 2 1 21h22L12 2Zm0 6 7.5 11.5h-15L12 8Zm-1 4v4h2v-4h-2Zm0 5v2h2v-2h-2Z" />
        </svg>
        {{ name }} недоступен
      </button>

      <Transition name="panel">
        <div v-if="open" class="panel" role="status">
          <strong>Не удалось связаться с {{ name }}</strong>
          <span>{{ hint }}</span>
          <span v-for="problem in providerStatus.problems" :key="problem" class="problem">{{ problem }}</span>
          <button class="retry" type="button" :disabled="providerStatus.checking" @click="checkProviderStatus">
            <span v-if="providerStatus.checking" class="spinner" aria-hidden="true"></span>
            {{ providerStatus.checking ? 'Проверка...' : 'Проверить снова' }}
          </button>
        </div>
      </Transition>
    </div>
  </Transition>
</template>

<style scoped>
.provider-badge {
  position: absolute;
  top: 14px;
  right: 14px;
  z-index: 3;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
}

.pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 11px;
  border-radius: 999px;
  border: 1px solid var(--gold);
  background: rgba(24, 22, 28, 0.92);
  color: var(--gold);
  font: inherit;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.18s ease, transform 0.18s ease, box-shadow 0.18s ease;
}

.pill:hover {
  background: rgba(44, 39, 30, 0.97);
  transform: translateY(-1px);
  box-shadow: 0 6px 18px -8px rgba(231, 200, 115, 0.45);
}

.pill:active {
  transform: translateY(0) scale(0.97);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 300px;
  padding: 12px 14px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: rgba(24, 22, 28, 0.96);
  color: var(--text);
  font-size: 13px;
  line-height: 1.35;
  box-shadow: var(--shadow-pop);
  transform-origin: top right;
}

.panel span {
  color: var(--text-muted);
}

.panel .problem {
  color: var(--text-faint);
  font-size: 12px;
}

.retry {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  align-self: flex-start;
  margin-top: 6px;
  padding: 7px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.18s ease, opacity 0.18s ease, transform 0.18s ease;
}

.retry:hover:not(:disabled) {
  background: var(--surface-hover);
}

.retry:active:not(:disabled) {
  transform: scale(0.97);
}

.retry:disabled {
  opacity: 0.7;
  cursor: default;
}

/* Turns only while a check runs */
.spinner {
  width: 11px;
  height: 11px;
  border-radius: 50%;
  border: 2px solid var(--border);
  border-top-color: var(--gold);
  animation: check-spin 0.7s linear infinite;
}

@keyframes check-spin {
  to { transform: rotate(360deg); }
}

.badge-enter-active,
.badge-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}

.badge-enter-from,
.badge-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

.panel-enter-active {
  transition: opacity 0.18s ease-out, transform 0.18s ease-out;
}

.panel-leave-active {
  transition: opacity 0.12s ease-in, transform 0.12s ease-in;
}

.panel-enter-from,
.panel-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.97);
}

@media (prefers-reduced-motion: reduce) {
  .pill,
  .retry,
  .badge-enter-active,
  .badge-leave-active,
  .panel-enter-active,
  .panel-leave-active {
    transition: none;
  }
}
</style>
