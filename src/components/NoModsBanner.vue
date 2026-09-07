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
  <Transition name="banner">
    <div v-if="visible" class="banner">
      <button class="close-btn" @click="dismiss">
        <svg viewBox="0 0 10 10" width="11" height="11"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3"/></svg>
      </button>
      <div class="banner-text">
        <strong>Играете без модов?</strong>
        <p>На сервере есть голосовой чат PlasmaVoice и другие удобства — попробуйте нашу сборку для оптимизации и комфортной игры</p>
      </div>
      <button class="cta-btn" @click="goToMods">Перейти к модам</button>
    </div>
  </Transition>
</template>

<style scoped>
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

.banner-enter-active,
.banner-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.banner-enter-from,
.banner-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

@media (prefers-reduced-motion: reduce) {
  .banner-enter-active,
  .banner-leave-active {
    transition: none;
  }
}
</style>
