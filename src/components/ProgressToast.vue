<script setup>
import { progressToastState, hideProgress } from '../store/progressToast.js';
</script>

<template>
  <Transition name="toast">
    <div v-if="progressToastState.visible" class="toast" :class="{ error: progressToastState.error }">
      <span v-if="!progressToastState.error" class="spinner"></span>
      <svg v-else viewBox="0 0 24 24" width="18" height="18" fill="currentColor" class="err-icon">
        <path d="M12 2 1 21h22L12 2Zm0 6 1 8h-2l1-8Zm0 10.5a1.25 1.25 0 1 1 0 2.5 1.25 1.25 0 0 1 0-2.5Z" />
      </svg>

      <div class="toast-body">
        <p class="toast-text">{{ progressToastState.text }}</p>
        <div v-if="!progressToastState.error" class="track">
          <div
            class="fill"
            :class="{ indeterminate: progressToastState.percent === null }"
            :style="progressToastState.percent !== null ? { width: progressToastState.percent + '%' } : {}"
          ></div>
        </div>
      </div>

      <span v-if="!progressToastState.error && progressToastState.percent !== null" class="percent mono">{{ progressToastState.percent }}%</span>
      <button v-if="progressToastState.error" class="dismiss" @click="hideProgress">
        <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3"/></svg>
      </button>
    </div>
  </Transition>
</template>

<style scoped>
.toast {
  position: fixed;
  left: 50%;
  bottom: 22px;
  transform: translateX(-50%);
  z-index: 200;
  display: flex;
  align-items: center;
  gap: 12px;
  width: min(380px, calc(100vw - 44px));
  padding: 12px 16px;
  border-radius: 12px;
  background: var(--surface);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-pop);
}
.toast.error {
  border-color: rgba(239, 68, 68, 0.4);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.16, 1, 0.3, 1);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(12px);
}

.spinner {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid var(--surface-2);
  border-top-color: var(--accent);
  animation: spin 0.7s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.err-icon {
  flex-shrink: 0;
  color: var(--bad);
}

.toast-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.toast-text {
  font-size: 13px;
  color: var(--text);
  overflow-wrap: anywhere;
}

.track {
  height: 5px;
  border-radius: 4px;
  background: var(--surface-2);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: 4px;
  background: var(--accent);
  transition: width 0.25s ease;
}

.fill.indeterminate {
  width: 40% !important;
  animation: progress-indeterminate 1.1s ease-in-out infinite;
}
@keyframes progress-indeterminate {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(250%);
  }
}

.percent {
  flex-shrink: 0;
  font-size: 11.5px;
  color: var(--text-faint);
}

.dismiss {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--text-faint);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.dismiss:hover {
  background: var(--surface-2);
  color: var(--text);
}

@media (prefers-reduced-motion: reduce) {
  .toast-enter-active,
  .toast-leave-active {
    transition: none;
  }
  .spinner,
  .fill.indeterminate {
    animation: none;
  }
}
</style>
