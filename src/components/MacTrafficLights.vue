<script setup>
defineProps({
  maximized: { type: Boolean, default: false },
});
defineEmits(['close', 'minimize', 'maximize']);
</script>

<template>
  <div class="lights">
    <button class="light light-close" title="Закрыть" @click="$emit('close')">
      <svg viewBox="0 0 10 10" width="6" height="6" fill="none"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" /></svg>
    </button>
    <button class="light light-minimize" title="Свернуть" @click="$emit('minimize')">
      <svg viewBox="0 0 10 10" width="6" height="6" fill="none"><path d="M1 5h8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" /></svg>
    </button>
    <button class="light light-maximize" :title="maximized ? 'Восстановить' : 'Развернуть'" @click="$emit('maximize')">
      <svg viewBox="0 0 10 10" width="6" height="6" fill="none"><path d="M2 8l6-6M8 8V2H2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
  </div>
</template>

<style scoped>
/* Shrink-wrapped: the empty rest of the parent row must stay a drag region and not trigger the hover glyphs */
.lights {
  display: flex;
  width: fit-content;
  align-items: center;
  gap: 8px;
  -webkit-app-region: no-drag;
}

.light {
  width: 12px;
  height: 12px;
  min-width: 12px;
  min-height: 12px;
  flex-shrink: 0;
  aspect-ratio: 1 / 1;
  border-radius: 50%;
  border: 0.5px solid rgba(0, 0, 0, 0.25);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  cursor: pointer;
  color: transparent;
  box-shadow: inset 0 1px 1px rgba(255, 255, 255, 0.15);
  transition: background 0.12s ease, color 0.12s ease;
}

/* Glyphs appear while the pointer is anywhere over the group, like the native buttons */
.lights:hover .light {
  color: rgba(0, 0, 0, 0.68);
}

.light-close {
  background: #ff5f56;
  border-color: #e0443e;
}
.light-close:hover {
  background: #ff6961;
}
.light-close:active {
  background: #bf4942;
}

.light-minimize {
  background: #ffbd2e;
  border-color: #dea123;
}
.light-minimize:hover {
  background: #ffc73d;
}
.light-minimize:active {
  background: #bf8e22;
}

.light-maximize {
  background: #27c93f;
  border-color: #1aab29;
}
.light-maximize:hover {
  background: #34d64c;
}
.light-maximize:active {
  background: #1d9630;
}
</style>
