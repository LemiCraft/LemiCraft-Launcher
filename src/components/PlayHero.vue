<script>
// Every title has to fit one line of the card at the narrowest window (about 26 letters), or the card changes
// its height. Short, warm, no full stop at the end.
const heroTitles = [
  // invitations
  'Готов залетать?',
  'Заходи, у нас уютно',
  'Заходи на огонёк!',
  'Ты как раз вовремя!',
  'Давай-давай, заходи!',
  'А вот и наш пушистик!',
  'Давно не виделись!',
  'Все в сборе, ждём тебя!',
  'Налей чаю и заходи',
  // longing
  'Сервер скучает без тебя',
  'Без тебя тут тихо',
  'Лемикрафт соскучился',
  'Пушистики уже заждались',
  'Кто-то ждёт тебя в игре',
  'Твой уголок ждёт тебя',
  'Пора вернуться домой!',
  'Домой, пушистик!',
  'Мурр, а ты где был?',
  // paws and tails
  'Лапки чешутся? Заходи!',
  'Лапки готовы? Поехали!',
  'Хвост трубой, и в игру!',
  'Пора размять лапки!',
  'Мир ждёт твои лапки!',
  'Хвостик виляет: заходи!',
  'Время мурчать и копать!',
  'Лапы в руки, и в игру!',
  // what to do
  'Что построим сегодня?',
  'Кого сегодня встретим?',
  'Пойдём что-нибудь построим',
  'Как насчёт приключения?',
  'Пора за приключениями!',
  'Тут пахнет приключениями',
  'Сегодня точно будет весело',
  'Пушистый день начинается',
  'Новый день, новые лапки',
];

// and some for the time of the day, picked about every other time
const timeTitles = {
  morning: ['Доброе утро, пушистик!', 'Утро начинается с игры', 'Кофе, лапки, Minecraft'],
  day: ['Хороший день для стройки', 'День в самом разгаре'],
  evening: ['Добрый вечер, лапки!', 'Вечер для приключений?', 'Вечерний заход?'],
  night: ['Не спится? Заходи!', 'Сова, это ты? Заходи!', 'Ночная смена в игре'],
};

function titlesForNow(hour) {
  if (hour >= 5 && hour < 11) return timeTitles.morning;
  if (hour >= 11 && hour < 17) return timeTitles.day;
  if (hour >= 17 && hour < 23) return timeTitles.evening;
  return timeTitles.night;
}

let previousHeroTitle = '';

function chooseHeroTitle(hour = new Date().getHours(), random = Math.random) {
  const pool = random() < 0.4 ? titlesForNow(hour) : heroTitles;
  const available = pool.filter((title) => title !== previousHeroTitle);
  previousHeroTitle = available[Math.floor(random() * available.length)];
  return previousHeroTitle;
}
</script>

<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { accountStore } from '../store/account.js';
import { serverStatus, startServerStatusPolling, recheckServerStatus } from '../store/serverStatus.js';
import { installState, startInstallListeners, launchGame, refreshInstalledState } from '../store/installProgress.js';
import { modsState } from '../store/mods.js';
import { startProviderStatusWatcher } from '../store/providerStatus.js';
import { profileView, onlineState, loadOnlinePlayers } from '../store/profile.js';
import ProviderBanner from './ProviderBanner.vue';

const emit = defineEmits(['navigate']);

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

// Hovering the online chip opens the list of everyone on the server; a click on a player opens their profile
const onlineOpen = ref(false);
let onlineCloseTimer = 0;
function showOnline() {
  clearTimeout(onlineCloseTimer);
  onlineOpen.value = true;
  loadOnlinePlayers();
}

// the edges of the list fade while there is more to scroll, as on the other pages
const onlineListEl = ref(null);
const onlineTopFaded = ref(false);
const onlineBottomFaded = ref(false);
function onOnlineScroll() {
  const el = onlineListEl.value;
  if (!el) return;
  onlineTopFaded.value = el.scrollTop > 2;
  onlineBottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 2;
}
// a short wait, so the pointer can cross the gap between the chip and the list
function hideOnlineSoon() {
  clearTimeout(onlineCloseTimer);
  onlineCloseTimer = setTimeout(() => { onlineOpen.value = false; }, 200);
}
function toggleOnline() {
  if (onlineOpen.value) onlineOpen.value = false;
  else showOnline();
}
const onlineNames = computed(() => {
  // the site knows everyone; until it answers, the few names the server's ping gave
  const names = onlineState.players.length
    ? onlineState.players
    : serverStatus.playersSample.map((player) => player?.name ?? '').filter((name) => VALID_NICKNAME.test(name));
  return [...new Set(names)].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }));
});
watch([onlineOpen, () => onlineNames.value.length], () => nextTick(onOnlineScroll));

function openPlayer(name) {
  onlineOpen.value = false;
  profileView.nick = name;
  emit('navigate', 'profile');
}
onUnmounted(() => clearTimeout(onlineCloseTimer));

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
        data-tour="play"
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
          <span class="online-wrap" data-tour="online" @mouseenter="showOnline" @mouseleave="hideOnlineSoon">
          <button type="button" class="chip online-chip" :aria-expanded="onlineOpen" aria-haspopup="true" @click="toggleOnline" @keydown.esc="onlineOpen = false">
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
              <span v-if="otherPlayersCount > 0" key="more" class="player-overflow">
                <span class="swap-slot">
                  <Transition name="chip-swap">
                    <span :key="otherPlayersCount" class="swap-item">+{{ otherPlayersCount }}</span>
                  </Transition>
                </span>
              </span>
            </TransitionGroup>
          </button>
          <Transition name="online-pop">
            <div v-if="onlineOpen && onlineNames.length" class="online-pop">
              <span class="online-pop-head">Сейчас в игре · {{ Math.max(serverStatus.players ?? 0, onlineNames.length) }}</span>
              <div ref="onlineListEl" class="online-pop-list edge-fade" :class="{ 'edge-top': onlineTopFaded, 'edge-bottom': onlineBottomFaded }" @scroll="onOnlineScroll">
                <button v-for="name in onlineNames" :key="name" type="button" class="online-row" :aria-label="`Профиль ${name}`" @click="openPlayer(name)">
                  <img class="online-row-face" :src="playerAvatarUrl({ name })" alt="" />
                  <span class="online-row-name">{{ name }}</span>
                </button>
              </div>
            </div>
          </Transition>
          </span>
          <span class="ping-wrap">
            <button type="button" class="chip mono ping-chip" :class="{ checking: serverStatus.checking }" aria-label="Проверить пинг заново" @click="recheckServerStatus">
              <span class="ping-dot" :class="[pingQuality(serverStatus.ping), { checking: serverStatus.checking }]"></span>
              <span class="swap-slot">
                <Transition name="chip-swap">
                  <span :key="serverStatus.ping" class="swap-item">{{ serverStatus.ping }}</span>
                </Transition>
              </span>
              мс
              <svg class="ping-redo" viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true"><path d="M12 4a8 8 0 1 0 7.75 6h-2.08A6 6 0 1 1 12 6V2l5 4-5 4V6Z" /></svg>
            </button>
            <!-- the hint: a small note over the chip, not the system tooltip, which appears under the pointer -->
            <span class="ping-hint">Нажмите, чтобы проверить заново</span>
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
  /* it grows a little instead of moving up: a button that leaves the pointer at its lower edge flickers */
  transition: background 0.25s ease, box-shadow 0.3s ease, transform 0.28s cubic-bezier(0.2, 0.7, 0.2, 1);
}
.play-btn:hover:not(:disabled) {
  background: var(--accent-hover);
  box-shadow: 0 14px 28px -10px rgba(255, 95, 95, 0.7);
  transform: scale(1.02);
}
.play-btn:active:not(:disabled) {
  transform: scale(0.985);
  transition-duration: 0.1s;
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
.online-chip {
  font-family: inherit;
  cursor: pointer;
  transition: background 0.2s ease, border-color 0.2s ease;
}
.online-chip:hover,
.online-chip[aria-expanded='true'] {
  background: rgba(34, 139, 70, 0.34);
  border-color: rgba(109, 226, 139, 0.55);
}
.online-wrap {
  position: relative;
  display: inline-flex;
}
.online-pop {
  position: absolute;
  left: 0;
  bottom: calc(100% + 8px);
  z-index: 6;
  width: 300px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  border-radius: 14px;
  background: rgba(17, 18, 22, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.14);
  box-shadow: 0 14px 34px -12px rgba(0, 0, 0, 0.7);
  transform-origin: 20px 100%;
}
/* the gap under the list belongs to it, so the pointer does not fall out on the way from the chip */
.online-pop::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: 100%;
  height: 10px;
}
.online-pop-head {
  padding: 0 4px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: rgba(255, 255, 255, 0.55);
}
.online-pop-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 2px;
  max-height: 116px;
  overflow-y: auto;
}
.online-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 5px 6px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: rgba(255, 255, 255, 0.92);
  font-family: inherit;
  font-size: 12.5px;
  font-weight: 600;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease;
}
.online-row:hover,
.online-row:focus-visible {
  background: rgba(255, 255, 255, 0.1);
}
.online-row-face {
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  border-radius: 6px;
  background: #17261c;
  object-fit: cover;
}
.online-row-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.online-pop-enter-active,
.online-pop-leave-active {
  transition: opacity 0.2s ease, transform 0.24s cubic-bezier(0.2, 0.7, 0.2, 1);
}
.online-pop-enter-from,
.online-pop-leave-to {
  opacity: 0;
  transform: translateY(6px) scale(0.97);
}
@media (prefers-reduced-motion: reduce) {
  .online-pop-enter-active,
  .online-pop-leave-active {
    transition: none;
  }
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

.ping-wrap {
  position: relative;
  display: inline-flex;
}
.ping-chip {
  cursor: pointer;
  font-family: var(--font-mono);
  transition: background 0.2s ease, border-color 0.2s ease;
}
.ping-chip:hover,
.ping-chip:focus-visible {
  background: rgba(0, 0, 0, 0.55);
  border-color: rgba(255, 255, 255, 0.4);
}
/* the arrow is there only while the pointer is, and it turns while the check is running */
.ping-redo {
  width: 0;
  margin-left: -6px;
  opacity: 0;
  transition: width 0.2s ease, margin 0.2s ease, opacity 0.2s ease;
}
.ping-chip:hover .ping-redo,
.ping-chip:focus-visible .ping-redo,
.ping-chip.checking .ping-redo {
  width: 12px;
  margin-left: 0;
  opacity: 0.8;
}
.ping-chip.checking .ping-redo {
  animation: ping-turn 0.8s linear infinite;
}
@keyframes ping-turn {
  to { transform: rotate(360deg); }
}
.ping-dot.checking {
  animation: ping-pulse 0.9s ease-in-out infinite;
}
@keyframes ping-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.25; }
}
.ping-hint {
  position: absolute;
  left: 50%;
  bottom: calc(100% + 8px);
  transform: translate(-50%, 4px);
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(17, 18, 22, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.14);
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.18s ease, transform 0.22s cubic-bezier(0.2, 0.7, 0.2, 1);
}
/* a moment after the pointer arrives, so that it does not flash when the pointer only passes by */
.ping-wrap:hover .ping-hint {
  opacity: 1;
  transform: translate(-50%, 0);
  transition-delay: 0.5s;
}
.ping-wrap:has(.checking) .ping-hint {
  opacity: 0;
  transition-delay: 0s;
}
@media (prefers-reduced-motion: reduce) {
  .ping-redo,
  .ping-hint {
    transition: none;
  }
  .ping-chip.checking .ping-redo,
  .ping-dot.checking {
    animation: none;
  }
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
