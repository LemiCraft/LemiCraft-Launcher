<script>
// every title has to fit one line of the card at the narrowest window, or the card changes its height
const heroTitles = [
  'Все в сборе, ждём тебя!',
  'Лапки чешутся? Заходи!',
  'Готов залетать?',
  'Пушистики уже заждались',
  'Мир ждёт твои лапки!',
  'Заходи, у нас уютно',
  'Пора за приключениями!',
  'Сервер скучает без тебя',
  'Пора вернуться домой!',
  'Хвост трубой, и в игру!',
  'Лапки готовы? Поехали!',
];
let previousHeroTitleIndex = -1;

function chooseHeroTitle() {
  const available = heroTitles.map((_, index) => index).filter((index) => index !== previousHeroTitleIndex);
  previousHeroTitleIndex = available[Math.floor(Math.random() * available.length)];
  return heroTitles[previousHeroTitleIndex];
}
</script>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { accountStore } from '../store/account.js';
import { serverStatus, startServerStatusPolling } from '../store/serverStatus.js';
import { installState, startInstallListeners, launchGame, refreshInstalledState } from '../store/installProgress.js';
import { modsState } from '../store/mods.js';
import { startProviderStatusWatcher } from '../store/providerStatus.js';
import ProviderBanner from './ProviderBanner.vue';

const bannerImages = Object.entries(import.meta.glob('../assets/banner*.jpg', { eager: true, query: '?url', import: 'default' }))
  .sort(([a], [b]) => a.localeCompare(b, undefined, { numeric: true }))
  .map(([, imageUrl]) => imageUrl);
const currentBanner = ref(0);
let bannerTimer;
const VALID_NICKNAME = /^[A-Za-z0-9_]{1,16}$/;
const heroTitle = ref(chooseHeroTitle());

// an avatar that did not load is left out instead of showing a broken picture; one that has loaded fades in
// over its placeholder circle instead of popping up
const failedAvatars = ref(new Set());
const loadedAvatars = ref(new Set());
function avatarFailed(player) {
  failedAvatars.value = new Set(failedAvatars.value).add(player.name);
}
function avatarLoaded(player) {
  loadedAvatars.value = new Set(loadedAvatars.value).add(player.name);
}
const visiblePlayers = computed(() =>
  serverStatus.playersSample
    // the list of a server can hold decorative lines instead of players (a motd in the player list): only real nicknames
    .filter((player) => VALID_NICKNAME.test(player?.name ?? ''))
    .filter((player, index, players) => players.findIndex((candidate) => candidate.name === player.name) === index)
    .filter((player) => !failedAvatars.value.has(player.name))
    .slice(0, 3),
);
const otherPlayersCount = computed(() => Math.max(0, (serverStatus.players ?? 0) - visiblePlayers.value.length));

function playerAvatarUrl(player) {
  return `https://lemicraft.ru/api/avatar/${encodeURIComponent(player.name)}?size=64`;
}

// the next image is decoded first, so the cross-fade never shows an empty card
function showNextBanner() {
  if (document.hidden || bannerImages.length < 2) return;
  const next = (currentBanner.value + 1) % bannerImages.length;
  const img = new Image();
  img.src = bannerImages[next];
  img.decode().then(() => { currentBanner.value = next; }).catch(() => {});
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
  startProviderStatusWatcher();
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
    <ProviderBanner />

    <div class="hero-main">
      <span class="eyebrow" :class="{ pending: !serverStatus.loaded }">
        <span class="dot" :class="{ off: serverStatus.loaded && !serverStatus.online }"></span>
        {{ !serverStatus.loaded ? 'Проверка сервера...' : serverStatus.online ? 'Сервер онлайн' : 'Сервер офлайн' }}
      </span>
      <h1>{{ heroTitle }}</h1>

      <button
        class="play-btn"
        :class="{ running: installState.isGameRunning }"
        :disabled="installState.isInstalling || installState.isGameRunning || modsState.applying || !accountStore.loggedIn"
        :title="modsState.applying ? 'Дождитесь завершения установки модов' : accountStore.checking ? 'Проверяю аккаунт' : !accountStore.loggedIn ? 'Сначала войдите в аккаунт' : ''"
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
                : accountStore.checking
                  ? 'Проверяю аккаунт...'
                  : !accountStore.loggedIn
                  ? 'Нужен вход в аккаунт'
                  : installState.isInstalled === false
                    ? 'Установить и играть'
                    : 'Играть'
        }}
      </button>

      <Transition name="chips" mode="out-in">
        <div v-if="serverStatus.loaded && serverStatus.online" key="online" class="chips">
          <span class="chip online-chip">
            <span class="online-dot"></span>
            <!-- a number that changes is cross-faded in its own place, so the chip does not twitch -->
            <span class="swap-slot">
              <Transition name="chip-swap">
                <span :key="serverStatus.players ?? 0" class="swap-item">{{ serverStatus.players ?? 0 }}</span>
              </Transition>
            </span>
            онлайн
            <TransitionGroup name="avatar" tag="span" class="player-avatar-stack" :class="{ filled: visiblePlayers.length }" aria-label="Игроки онлайн">
              <span
                v-for="player in visiblePlayers"
                :key="player.id || player.name"
                class="player-avatar"
                :title="player.name"
              >
                <img
                  class="player-avatar-img"
                  :class="{ ready: loadedAvatars.has(player.name) }"
                  :src="playerAvatarUrl(player)"
                  :alt="player.name"
                  @load="avatarLoaded(player)"
                  @error="avatarFailed(player)"
                />
              </span>
              <span v-if="otherPlayersCount > 0" key="more" class="player-overflow" :title="`Ещё ${otherPlayersCount} игроков`">
                <span class="swap-slot">
                  <Transition name="chip-swap">
                    <span :key="otherPlayersCount" class="swap-item">+{{ otherPlayersCount }}</span>
                  </Transition>
                </span>
              </span>
            </TransitionGroup>
          </span>
          <span class="chip mono">
            <span class="ping-dot" :class="pingQuality(serverStatus.ping)"></span>
            <span class="swap-slot">
              <Transition name="chip-swap">
                <span :key="serverStatus.ping" class="swap-item">{{ serverStatus.ping }}</span>
              </Transition>
            </span>
            мс
          </span>
          <span class="chip">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><path d="M12 2 2 7l10 5 10-5-10-5Zm0 8.5L4.5 7 12 3.5 19.5 7 12 10.5ZM4 9.5v6.7L12 20l8-3.8V9.5l-8 4-8-4Z"/></svg>
            MC {{ serverStatus.version }}
          </span>
        </div>
        <div v-else-if="!serverStatus.loaded" key="loading" class="chips">
          <span class="chip-skeleton online-skeleton"></span>
          <span class="chip-skeleton"></span>
          <span class="chip-skeleton short"></span>
        </div>
      </Transition>
    </div>

    <Transition name="banner">
      <div :key="currentBanner" class="hero-banner" aria-hidden="true">
        <div class="hero-banner-img" :style="{ backgroundImage: `url(${bannerImages[currentBanner]})` }"></div>
      </div>
    </Transition>
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
.eyebrow.pending { color: var(--text-faint); }
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
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-shadow: 0 2px 16px rgba(0,0,0,.55);
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
  /* the row keeps its height while loading and when the server is offline */
  min-height: 28px;
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
  height: 28px;
  box-sizing: border-box;
  padding: 0 11px;
  border-radius: 999px;
  animation: chip-in 0.4s ease both;
}
@keyframes chip-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
.chip.mono {
  font-family: var(--font-mono);
}
.online-chip {
  color: #a8f0b8;
  background: rgba(34, 139, 70, 0.22);
  border-color: rgba(109, 226, 139, 0.34);
  font-weight: 650;
}
.online-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #54e47c;
  box-shadow: 0 0 0 3px rgba(84, 228, 124, 0.17);
}
.player-avatar-stack {
  display: inline-flex;
  align-items: center;
  padding-left: 0;
  transition: padding-left 0.35s ease;
}
.player-avatar-stack.filled {
  padding-left: 3px;
}
.player-avatar {
  width: 20px;
  height: 20px;
  max-width: 22px;
  flex: 0 0 20px;
  margin-left: -4px;
  box-sizing: border-box;
  border: 1.5px solid #225a37;
  border-radius: 50%;
  background: #17261c;
  overflow: hidden;
}
.player-avatar-img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  opacity: 0;
  transition: opacity 0.35s ease;
}
.player-avatar-img.ready {
  opacity: 1;
}
/* an avatar grows in and out of the stack, and the chip widens with it */
.avatar-enter-active,
.avatar-leave-active {
  transition: max-width 0.35s ease, min-width 0.35s ease, margin-left 0.35s ease, padding 0.35s ease, opacity 0.3s ease;
}
.player-avatar-stack > .avatar-enter-from,
.player-avatar-stack > .avatar-leave-to {
  max-width: 0;
  min-width: 0;
  margin-left: 0;
  padding: 0;
  opacity: 0;
}

/* a changing number: the old one fades up, the new one comes from below, both in the same place */
.swap-slot {
  display: inline-grid;
}
.swap-slot > .swap-item {
  grid-area: 1 / 1;
  text-align: center;
}
.chip-swap-enter-active,
.chip-swap-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}
.chip-swap-enter-from {
  opacity: 0;
  transform: translateY(5px);
}
.chip-swap-leave-to {
  opacity: 0;
  transform: translateY(-5px);
}

/* the row of chips replaces the placeholders with a short fade instead of at once */
.chips-enter-active,
.chips-leave-active {
  transition: opacity 0.18s ease;
}
.chips-enter-from,
.chips-leave-to {
  opacity: 0;
}
.player-overflow {
  position: relative;
  z-index: 1;
  max-width: 40px;
  overflow: hidden;
  min-width: 22px;
  height: 22px;
  padding: 0 4px;
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  margin-left: -3px;
  padding: 0;
  border: 1.5px solid #225a37;
  border-radius: 50%;
  background: #174429;
  color: #baf4c7;
  font-size: 10px;
  font-weight: 700;
  line-height: 1;
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
  height: 28px;
  border-radius: 999px;
  background: var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}
.chip-skeleton.short {
  width: 64px;
}
.chip-skeleton.online-skeleton {
  width: 104px;
}

.hero-banner {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
}
.hero-banner-img {
  position: absolute;
  inset: 0;
  background-position: center right;
  background-size: cover;
  /* a slow drift for as long as the banner stays, then the next one takes over */
  animation: banner-drift 30s ease-out both;
}
@keyframes banner-drift {
  from { transform: scale(1); }
  to { transform: scale(1.06) translateX(-1.2%); }
}

.banner-enter-active,
.banner-leave-active {
  transition: opacity 1.1s ease, transform 1.1s cubic-bezier(.2, .75, .25, 1);
}
.banner-enter-from {
  opacity: 0;
  transform: scale(1.025);
}
.banner-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .banner-enter-active,
  .banner-leave-active { transition: none; }
  .hero-banner-img { animation: none; }
}
</style>
