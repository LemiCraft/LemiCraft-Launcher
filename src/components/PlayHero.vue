<script setup>
import { onMounted, onUnmounted, ref } from 'vue';
import { accountStore } from '../store/account.js';
import { serverStatus, startServerStatusPolling } from '../store/serverStatus.js';
import { installState, startInstallListeners, launchGame, refreshInstalledState } from '../store/installProgress.js';
import { modsState } from '../store/mods.js';

const bannerImages = Object.entries(import.meta.glob('../assets/banner*.png', { eager: true, query: '?url', import: 'default' }))
  .sort(([a], [b]) => a.localeCompare(b, undefined, { numeric: true }))
  .map(([, imageUrl]) => imageUrl);
const currentBanner = ref(0);
let bannerTimer;

function showNextBanner() {
  currentBanner.value = (currentBanner.value + 1) % bannerImages.length;
}

function pingQuality(ms) {
  if (ms == null) return 'unknown';
  if (ms <= 150) return 'good';
  if (ms <= 300) return 'ok';
  return 'bad';
}

onMounted(() => {
  bannerTimer = window.setInterval(showNextBanner, 30_000);
  startServerStatusPolling();
  startInstallListeners();
  refreshInstalledState();
});

onUnmounted(() => {
  window.clearInterval(bannerTimer);
});
</script>

<template>
  <div class="hero">
    <div class="hero-glow"></div>

    <div class="hero-main">
      <span class="eyebrow" :class="{ pending: !serverStatus.loaded }">
        <span class="dot" :class="{ off: serverStatus.loaded && !serverStatus.online }"></span>
        {{ !serverStatus.loaded ? 'Проверка сервера...' : serverStatus.online ? 'Сервер онлайн' : 'Сервер офлайн' }}
      </span>
      <h1>Готов залетать?</h1>
      <p class="sub">
        LemiCraft<template v-if="serverStatus.loaded && serverStatus.online"> · {{ serverStatus.players }} игроков сейчас в игре</template>
      </p>

      <button
        class="play-btn"
        :class="{ running: installState.isGameRunning }"
        :disabled="installState.isInstalling || installState.isGameRunning || modsState.applying || !accountStore.loggedIn"
        :title="modsState.applying ? 'Дождитесь завершения установки модов' : !accountStore.loggedIn ? 'Сначала войдите в аккаунт' : ''"
        @click="launchGame"
      >
        <svg v-if="!installState.isInstalling && !installState.isGameRunning" viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M8 5v14l11-7Z" /></svg>
        <span v-else-if="installState.isInstalling" class="spinner"></span>
        <span v-else class="running-dot"></span>
        {{
          installState.isInstalling
            ? 'Загрузка...'
            : installState.isGameRunning
              ? 'Игра запущена'
              : modsState.applying
                ? 'Применяются моды...'
                : !accountStore.loggedIn
                  ? 'Нужен вход в аккаунт'
                  : installState.isInstalled === false
                    ? 'Установить и играть'
                    : 'Играть'
        }}
      </button>

      <div v-if="serverStatus.loaded && serverStatus.online" class="chips">
        <span class="chip">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><path d="M12 2 2 7l10 5 10-5-10-5Zm0 8.5L4.5 7 12 3.5 19.5 7 12 10.5ZM4 9.5v6.7L12 20l8-3.8V9.5l-8 4-8-4Z"/></svg>
          MC {{ serverStatus.version }}
        </span>
        <span class="chip mono">
          <span class="ping-dot" :class="pingQuality(serverStatus.ping)"></span>
          {{ serverStatus.ping }} мс
        </span>
      </div>
      <div v-else-if="!serverStatus.loaded" class="chips">
        <span class="chip-skeleton"></span>
        <span class="chip-skeleton short"></span>
      </div>
    </div>

    <div :key="currentBanner" class="hero-banner" :style="{ backgroundImage: `url(${bannerImages[currentBanner]})` }" aria-hidden="true"></div>
  </div>
</template>

<style scoped>
.hero {
  position: relative;
  border-radius: var(--radius-lg);
  background: #08090b;
  isolation: isolate;
  border: 1px solid var(--border);
  padding: 34px 36px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  overflow: hidden;
  box-shadow: var(--shadow-card);
}

.hero::after {
  content: '';
  position: absolute;
  inset: 0;
  z-index: 0;
  background: linear-gradient(90deg, rgba(0,0,0,.72) 0%, rgba(0,0,0,.58) 27%, rgba(0,0,0,.28) 72%, rgba(0,0,0,.08) 100%), linear-gradient(0deg, rgba(0,0,0,.24), transparent 55%);
  pointer-events: none;
}

@keyframes banner-reveal {
  from { opacity: 0; transform: scale(1.025); }
  to { opacity: 1; transform: scale(1); }
}

.hero-glow {
  position: absolute;
  top: -60%;
  right: -10%;
  width: 480px;
  height: 480px;
  background: radial-gradient(circle, rgba(255, 95, 95, 0.22), transparent 65%);
  pointer-events: none;
  z-index: 0;
}

.hero-main {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;
}

.eyebrow {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
  font-weight: 600;
  color: #9df0b1;
  letter-spacing: 0.02em;
  text-transform: uppercase;
}

.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--good);
  box-shadow: 0 0 0 3px var(--good-soft);
}
.dot.off {
  background: var(--bad);
  box-shadow: 0 0 0 3px var(--bad-soft);
}

.eyebrow.pending {
  color: var(--text-faint);
}
.eyebrow.pending .dot {
  background: var(--text-faint);
  box-shadow: 0 0 0 3px var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

h1 {
  font-size: 34px;
  font-weight: 800;
  color: #fff;
  margin-top: 2px;
  text-shadow: 0 2px 16px rgba(0,0,0,.55);
}

.sub {
  color: rgba(255,255,255,.82);
  font-size: 14.5px;
  margin-bottom: 6px;
}

.play-btn {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-top: 6px;
  padding: 13px 28px;
  border-radius: 12px;
  border: none;
  background: var(--accent);
  color: var(--accent-text-on);
  font-family: var(--font-display);
  font-size: 15.5px;
  font-weight: 700;
  cursor: pointer;
  box-shadow: 0 12px 24px -10px rgba(255, 95, 95, 0.55);
  transition: background 0.15s ease, transform 0.1s ease;
}
.play-btn:hover {
  background: var(--accent-hover);
  transform: translateY(-1px);
}
.play-btn:active {
  transform: translateY(0);
}
.play-btn:disabled {
  opacity: 0.75;
  cursor: default;
  transform: none;
}

.spinner {
  width: 15px;
  height: 15px;
  border-radius: 50%;
  border: 2px solid rgba(26, 15, 15, 0.35);
  border-top-color: var(--accent-text-on);
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.play-btn.running {
  background: var(--surface-2);
  color: var(--text-muted);
  box-shadow: none;
}

.running-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--good);
  box-shadow: 0 0 0 3px var(--good-soft);
}

.chips {
  display: flex;
  gap: 8px;
  margin-top: 18px;
  flex-wrap: wrap;
}

.chip {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  font-weight: 500;
  color: rgba(255,255,255,.9);
  background: rgba(0,0,0,.42);
  border: 1px solid rgba(255,255,255,.22);
  padding: 6px 11px;
  backdrop-filter: blur(8px);
  border-radius: 999px;
}
.chip.mono {
  font-family: var(--font-mono);
}

.ping-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-faint);
}
.ping-dot.good {
  background: var(--good);
  box-shadow: 0 0 0 2px var(--good-soft);
}
.ping-dot.ok {
  background: var(--gold);
  box-shadow: 0 0 0 2px var(--gold-soft);
}
.ping-dot.bad {
  background: var(--bad);
  box-shadow: 0 0 0 2px var(--bad-soft);
}

.chip-skeleton {
  width: 90px;
  height: 26px;
  border-radius: 999px;
  background: var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}
.chip-skeleton.short {
  width: 64px;
}

.hero-banner {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  animation: banner-reveal 1.1s cubic-bezier(.2,.75,.25,1) both;
  background-position: center right;
  background-size: cover;
}

@media (prefers-reduced-motion: reduce) {
  .hero-banner { animation: none; }
}
</style>
