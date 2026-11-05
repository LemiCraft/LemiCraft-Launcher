<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';

const props = defineProps({
  value: { type: Number, default: null },
  format: { type: Function, required: true },
  delay: { type: Number, default: 0 },
  duration: { type: Number, default: 800 },
});

// The number runs up from zero when it first shows, and from the old value when a refresh brings a new one
const shown = ref(0);
let frame = 0;
let timer = 0;

const reduced = () => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

function run(from, to, delay, duration) {
  cancelAnimationFrame(frame);
  clearTimeout(timer);
  if (from === to || reduced()) {
    shown.value = to;
    return;
  }
  shown.value = from;
  timer = setTimeout(() => {
    const started = performance.now();
    const step = (now) => {
      const t = Math.min(1, (now - started) / duration);
      const eased = 1 - Math.pow(1 - t, 3);
      shown.value = from + (to - from) * eased;
      if (t < 1) frame = requestAnimationFrame(step);
      else shown.value = to;
    };
    frame = requestAnimationFrame(step);
  }, delay);
}

onMounted(() => {
  if (props.value == null) return;
  run(0, props.value, props.delay, props.duration);
});

watch(
  () => props.value,
  (next, previous) => {
    if (next == null) shown.value = 0;
    else run(previous == null ? 0 : shown.value, next, 0, 500);
  },
);

onBeforeUnmount(() => {
  cancelAnimationFrame(frame);
  clearTimeout(timer);
});

const text = computed(() => (props.value == null ? props.format(null) : props.format(Math.round(shown.value))));
</script>

<template>
  <span class="count">{{ text }}</span>
</template>

<style scoped>
.count {
  font-variant-numeric: tabular-nums;
}
</style>
