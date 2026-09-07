<script setup>
import { onMounted } from 'vue';
import { accountStore } from '../store/account.js';
import { serverStatus, startServerStatusPolling } from '../store/serverStatus.js';
import { installState, startInstallListeners, launchGame, refreshInstalledState } from '../store/installProgress.js';
import { modsState } from '../store/mods.js';

function pingQuality(ms) {
  if (ms == null) return 'unknown';
  if (ms <= 150) return 'good';
  if (ms <= 300) return 'ok';
  return 'bad';
}

onMounted(() => {
  startServerStatusPolling();
  startInstallListeners();
  refreshInstalledState();
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

    <div class="hero-art" aria-hidden="true">
      <div class="art-blob blob-1"></div>
      <div class="art-blob blob-2"></div>
      <svg class="art-paw" viewBox="0 0 24 24" width="120" height="120" fill="currentColor">
        <path d="M8.5 9c1 0 1.8-1 1.8-2.5S9.5 4 8.5 4s-1.8 1-1.8 2.5S7.5 9 8.5 9Zm7 0c1 0 1.8-1 1.8-2.5S16.5 4 15.5 4s-1.8 1-1.8 2.5S14.5 9 15.5 9ZM4.8 13c.8 0 1.5-.9 1.5-2s-.7-2-1.5-2-1.5.9-1.5 2 .7 2 1.5 2Zm14.4 0c.8 0 1.5-.9 1.5-2s-.7-2-1.5-2-1.5.9-1.5 2 .7 2 1.5 2ZM12 11c-2.8 0-5.5 1.9-5.5 5 0 2.2 2.3 3.5 5.5 3.5s5.5-1.3 5.5-3.5c0-3.1-2.7-5-5.5-5Z" />
      </svg>
    </div>
  </div>
</template>

<style scoped>
.hero {
  position: relative;
  border-radius: var(--radius-lg);
  background: linear-gradient(160deg, var(--surface) 0%, #1a1720 60%, #1c1620 100%);
  border: 1px solid var(--border);
  padding: 34px 36px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  overflow: hidden;
  box-shadow: var(--shadow-card);
}

.hero-glow {
  position: absolute;
  top: -60%;
  right: -10%;
  width: 480px;
  height: 480px;
  background: radial-gradient(circle, rgba(255, 95, 95, 0.22), transparent 65%);
  pointer-events: none;
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
  color: var(--good);
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
  color: var(--text);
  margin-top: 2px;
}

.sub {
  color: var(--text-muted);
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
  color: var(--text-muted);
  background: var(--surface-2);
  border: 1px solid var(--border);
  padding: 6px 11px;
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

.hero-art {
  position: relative;
  z-index: 1;
  width: 200px;
  height: 180px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.art-paw {
  color: var(--surface-2);
  opacity: 0.9;
  transform: rotate(-8deg);
}

.art-blob {
  position: absolute;
  border-radius: 50%;
  filter: blur(2px);
}
.blob-1 {
  width: 140px;
  height: 140px;
  background: radial-gradient(circle, rgba(231, 200, 115, 0.16), transparent 70%);
  top: -10px;
  right: 10px;
}
.blob-2 {
  width: 90px;
  height: 90px;
  background: radial-gradient(circle, rgba(255, 95, 95, 0.18), transparent 70%);
  bottom: -10px;
  left: 20px;
}

@media (max-width: 760px) {
  .hero-art {
    display: none;
  }
}
</style>
