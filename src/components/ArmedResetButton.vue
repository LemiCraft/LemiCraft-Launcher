<script setup>
import { ref } from 'vue';

defineProps({
  title: { type: String, default: 'Сбросить к значению по умолчанию' },
  armedLabel: { type: String, default: 'Точно?' },
  disabled: { type: Boolean, default: false },
});
const emit = defineEmits(['confirm']);

const armed = ref(false);
const done = ref(false);
let timer = null;

function onClick() {
  if (!armed.value) {
    armed.value = true;
    clearTimeout(timer);
    timer = setTimeout(() => (armed.value = false), 3000);
    return;
  }
  clearTimeout(timer);
  armed.value = false;
  done.value = true;
  // Delayed — a caller whose @confirm unmounts this via v-if would cut off the done checkmark before it paints
  setTimeout(() => emit('confirm'), 220);
  timer = setTimeout(() => (done.value = false), 1100);
}
</script>

<template>
  <button class="reset-btn" :class="{ armed, done }" :disabled="disabled" :title="title" @click="onClick">
    <svg v-if="!done" viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 5V2L7 6l5 4V7c3.3 0 6 2.7 6 6s-2.7 6-6 6-6-2.7-6-6H4c0 4.4 3.6 8 8 8s8-3.6 8-8-3.6-8-8-8Z"/></svg>
    <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z"/></svg>
    <span class="reset-label" :class="{ shown: armed }">{{ armedLabel }}</span>
  </button>
</template>

<style scoped>
.reset-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 9px 10px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text-faint);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s ease, color 0.2s ease, border-color 0.2s ease;
}
.reset-btn:disabled {
  opacity: 0.6;
  cursor: default;
}
.reset-btn svg {
  transition: transform 0.25s ease;
  flex-shrink: 0;
}
.reset-btn.armed svg {
  transform: rotate(-75deg);
}
.reset-btn:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--text);
}
.reset-btn.armed {
  color: var(--bad);
  border-color: rgba(239, 68, 68, 0.4);
  background: var(--bad-soft);
}
.reset-btn.done {
  color: var(--good);
  border-color: rgba(34, 197, 94, 0.4);
  background: var(--good-soft);
}

.reset-label {
  display: inline-block;
  max-width: 0;
  margin-left: 0;
  opacity: 0;
  overflow: hidden;
  white-space: nowrap;
  transition: max-width 0.25s ease, opacity 0.2s ease, margin-left 0.25s ease;
}
.reset-label.shown {
  max-width: 60px;
  margin-left: 6px;
  opacity: 1;
}
</style>
