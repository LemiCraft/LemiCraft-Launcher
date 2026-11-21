<script>
// Module scope (not script setup state) so dismissal survives HomeView remount, but only for this session
let dismissedThisSession = false;
</script>

<script setup>
import { onMounted, ref, watch } from 'vue';
import { modsState, hasNoMods, triggerPackHighlight } from '../store/mods.js';

const emit = defineEmits(['navigate']);

const visible = ref(false);

onMounted(async () => {
  if (!window.__TAURI_INTERNALS__) return;
  // Skip while an install is in flight — the file-based check below would still read as "no mods"
  if (dismissedThisSession || modsState.applying) return;
  visible.value = await hasNoMods();
});

watch(
  () => modsState.applying,
  (applying) => {
    if (applying) visible.value = false;
  },
);

function goToMods() {
  triggerPackHighlight();
  emit('navigate', 'mods');
}

function dismiss() {
  visible.value = false;
  dismissedThisSession = true;
}
</script>

<template>
  <!-- the place is always there and opens and closes in height, so that the news below slide instead of jumping -->
  <div class="banner-slot" :class="{ open: visible }">
    <div class="banner-clip">
      <div class="banner">
        <button class="close-btn" @click="dismiss">
          <svg viewBox="0 0 10 10" width="11" height="11"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3"/></svg>
        </button>
        <div class="banner-text">
          <strong>Играете без модов?</strong>
          <p>На сервере есть голосовой чат PlasmaVoice и другие удобства — попробуйте нашу сборку для оптимизации и комфортной игры</p>
        </div>
        <button class="cta-btn" @click="goToMods">Перейти к модам</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* closed it has no height, and a negative margin takes back the gap of the page that it would otherwise leave */
.banner-slot {
  display: grid;
  grid-template-rows: 0fr;
  margin-bottom: -20px;
  opacity: 0;
  transition: grid-template-rows var(--d-slow) var(--ease-move), margin-bottom var(--d-slow) var(--ease-move), opacity var(--d-base) ease;
}
.banner-slot.open {
  grid-template-rows: 1fr;
  margin-bottom: 0;
  opacity: 1;
}
.banner-clip {
  min-height: 0;
  overflow: hidden;
  visibility: hidden;
  transition: visibility 0s var(--d-slow);
}
.banner-slot.open .banner-clip {
  visibility: visible;
  transition-delay: 0s;
}

.banner {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 14px 40px 14px 18px;
  background: var(--accent-soft);
  border: 1px solid var(--accent);
  border-radius: var(--radius-md);
  position: relative;
}

.banner-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.banner-text strong {
  font-size: 14px;
  color: var(--text);
}
.banner-text p {
  font-size: 12.5px;
  color: var(--text-muted);
  line-height: 1.4;
}

.cta-btn {
  flex-shrink: 0;
  padding: 9px 16px;
  border-radius: 8px;
  border: none;
  background: var(--accent);
  color: var(--accent-text-on);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.15s ease;
}
.cta-btn:hover {
  background: var(--accent-hover);
}

.close-btn {
  position: absolute;
  top: 10px;
  right: 10px;
  width: 24px;
  height: 24px;
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


</style>
