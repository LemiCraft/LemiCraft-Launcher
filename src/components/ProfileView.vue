<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { accountStore } from '../store/account.js';
import { profileState, loadProfile } from '../store/profile.js';
import { copyText } from '../utils/clipboard.js';
import { pushNotification } from '../store/notifications.js';
import { formatAgo, formatCount, formatDate, formatDuration, formatLastLogin, formatPing, roleBackground, roleTextColor } from '../utils/profileFormat.js';
import SkinViewer3D from './SkinViewer3D.vue';
import CountValue from './CountValue.vue';

const SITE = 'https://lemicraft.ru';

const profile = computed(() => profileState.profile);
const stats = computed(() => profile.value?.stats ?? null);

const presence = computed(() => {
  const p = profile.value;
  if (!p) return '';
  if (p.online) return 'Сейчас в игре';
  const ago = formatAgo(p.lastSeen);
  return ago ? `Был в игре ${ago}` : '';
});

const banText = computed(() => {
  const p = profile.value;
  if (!p?.banned) return '';
  if (p.banPermanent) return 'Заблокирован навсегда';
  return p.banEndsAt ? `Заблокирован до ${formatDate(p.banEndsAt)}` : 'Заблокирован';
});

// the order and the names of the site's statistics
const cells = computed(() => {
  const s = stats.value;
  return [
    { id: 'registered', label: 'Играет с', value: formatDate(s?.registered), icon: 'M7 3v3M17 3v3M4 8h16M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Z' },
    { id: 'lastLogin', label: 'Последний вход', value: formatLastLogin(s?.lastLogin), icon: 'M12 7v5l3 2M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z' },
    { id: 'playtime', label: 'Время в игре', num: s?.playtime, format: formatDuration, icon: 'M7 3h10M7 21h10M8 3v3a4 4 0 0 0 1.6 3.2L12 12l-2.4 2.8A4 4 0 0 0 8 18v3M16 3v3a4 4 0 0 1-1.6 3.2L12 12l2.4 2.8A4 4 0 0 1 16 18v3' },
    { id: 'sessions', label: 'Сессий', num: s?.sessions, format: formatCount, icon: 'M14 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4M10 8l4 4-4 4M14 12H4' },
    { id: 'deaths', label: 'Смертей', num: s?.deaths, format: formatCount, icon: 'M12 3a7 7 0 0 0-7 7c0 2.4 1.2 4.1 3 5.2V18a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2.8c1.8-1.1 3-2.8 3-5.2a7 7 0 0 0-7-7ZM9.5 11h.01M14.5 11h.01' },
    { id: 'mobKills', label: 'Убито мобов', num: s?.mobKills, format: formatCount, icon: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z' },
    { id: 'playerKills', label: 'Убито игроков', num: s?.playerKills, format: formatCount, icon: 'M14.5 5.5 18.5 9.5M5 19l9-9M6 14l4 4M15 4l5 5-2 2-5-5Z' },
    { id: 'ping', label: 'Средний пинг', num: s?.ping, format: formatPing, icon: 'M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5h.01' },
  ];
});

// no figures yet: the page says why instead of showing zeros
const statsNote = computed(() => {
  if (!profile.value || stats.value) return '';
  return profile.value.complete ? 'Статистика сейчас недоступна' : 'Статистика появится после обновления сайта';
});

// A failure is not the player's fault: say what happened in a sentence, keep the raw reason small
const lastError = ref('');
watch(() => profileState.error, (error) => { if (error) lastError.value = error; }, { immediate: true });
const errorView = computed(() => {
  const raw = lastError.value;
  if (raw.includes('белом списке')) {
    return { title: 'Профиля пока нет', text: 'Этого аккаунта нет в белом списке сервера, поэтому профиль показать не из чего.', detail: '' };
  }
  if (raw.includes('Слишком много')) {
    return { title: 'Подождите немного', text: 'Профиль запрашивали слишком часто. Попробуйте через минуту.', detail: '' };
  }
  return { title: 'Профиль не загрузился', text: 'Сайт сейчас не отвечает. Проверьте соединение и попробуйте снова.', detail: raw };
});

const skinReady = computed(() => !!accountStore.skinUrl);

// A window too small for the figure goes without it (and without the 3D view running for nothing)
const roomForSkin = ref(true);
let roomQuery = null;
const onRoomChange = (event) => { roomForSkin.value = event.matches; };
onMounted(() => {
  roomQuery = window.matchMedia?.('(min-width: 1161px) and (min-height: 681px)') ?? null;
  if (!roomQuery) return;
  roomForSkin.value = roomQuery.matches;
  roomQuery.addEventListener('change', onRoomChange);
});
onBeforeUnmount(() => roomQuery?.removeEventListener('change', onRoomChange));
// the figure fades in over its placeholder once the skin is on the model
const skinLoaded = ref(false);
watch(() => accountStore.skinUrl, () => { skinLoaded.value = false; });

// the button itself answers a copy: the icon turns into a tick for a moment
const copied = ref(false);
let copiedTimer = 0;
async function copyUuid() {
  if (!profile.value?.uuid) return;
  if (await copyText(profile.value.uuid)) {
    copied.value = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => { copied.value = false; }, 1800);
  } else {
    pushNotification('Не удалось скопировать', 'error');
  }
}
onBeforeUnmount(() => clearTimeout(copiedTimer));

// Fade of the edges while there is more to scroll, as on the other pages
const scrollEl = ref(null);
const topFaded = ref(false);
const bottomFaded = ref(false);
function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  topFaded.value = el.scrollTop > 4;
  bottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 4;
}

// The page shrinks to the window instead of scrolling; only past the smallest scale it scrolls
const MIN_SCALE = 0.7;
const SCROLL_PADDING = 8;
const fitEl = ref(null);
const scale = ref(1);
const natural = ref(0);
let sizeObserver = null;

// The width the page is laid out at grows as it shrinks, and a wider page can be shorter (the statistics take
// fewer rows), which asks for a bigger scale again: left alone that flips for ever and the page twitches.
// So the steps are counted per window size, a step back to where it was settles on the smaller scale,
// and then it stays until the window (or the content) really changes.
const MAX_STEPS = 5;
let boxKey = '';
let steps = [];
let settledHeight = 0;
let measureFrame = 0;

function measure() {
  measureFrame = 0;
  const box = scrollEl.value;
  const fit = fitEl.value;
  if (!box || !fit) return;
  const available = box.clientHeight - SCROLL_PADDING;
  const height = fit.offsetHeight;
  natural.value = height;
  if (available <= 0 || height <= 0) return;

  // offsetWidth, not clientWidth: the scrollbar that comes and goes must not look like a new window
  const key = `${box.offsetWidth}x${box.clientHeight}`;
  if (key !== boxKey || (settledHeight && Math.abs(height - settledHeight) > 40)) {
    boxKey = key;
    steps = [];
    settledHeight = 0;
  }
  if (settledHeight) return;

  let next = scale.value;
  if (height * scale.value > available + 1) next = Math.max(MIN_SCALE, available / height);
  else if (scale.value < 1 && height * scale.value < available - 1) next = Math.min(1, available / height);
  if (Math.abs(next - scale.value) <= 0.01) {
    nextTick(onScroll);
    return;
  }
  const flipped = steps.length >= 2 && Math.abs(next - steps[steps.length - 2]) < 0.02;
  if (flipped || steps.length >= MAX_STEPS) {
    settledHeight = height;
    scale.value = Math.min(scale.value, next);
  } else {
    steps.push(scale.value);
    scale.value = next;
  }
  nextTick(onScroll);
}

// one measurement per frame, however many observers fire
function scheduleMeasure() {
  if (!measureFrame) measureFrame = requestAnimationFrame(measure);
}

const fitStyle = computed(() =>
  scale.value < 1 ? { transform: `scale(${scale.value})`, transformOrigin: '0 0', width: `${100 / scale.value}%` } : {},
);
const boxStyle = computed(() => (scale.value < 1 ? { height: `${natural.value * scale.value}px` } : {}));

onMounted(() => {
  loadProfile();
  if (typeof ResizeObserver === 'undefined') return;
  sizeObserver = new ResizeObserver(scheduleMeasure);
  if (scrollEl.value) sizeObserver.observe(scrollEl.value);
  if (fitEl.value) sizeObserver.observe(fitEl.value);
});
onBeforeUnmount(() => {
  sizeObserver?.disconnect();
  cancelAnimationFrame(measureFrame);
});
</script>

<template>
  <div class="profile">
    <div class="head">
      <h1>Профиль</h1>
      <button class="refresh" :disabled="profileState.loading" @click="loadProfile({ force: true })">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 4a8 8 0 1 0 7.75 6h-2.08A6 6 0 1 1 12 6V2l5 4-5 4V6Z" /></svg>
        {{ profileState.loading ? 'Обновляю...' : 'Обновить' }}
      </button>
    </div>

    <div class="scroll-wrap">
      <div ref="scrollEl" class="scroll edge-fade" :class="{ 'edge-top': topFaded, 'edge-bottom': bottomFaded }" @scroll="onScroll">
      <div class="fit-box" :style="boxStyle">
      <div ref="fitEl" class="fit" :style="fitStyle">
      <Transition name="swap" mode="out-in">
      <section v-if="profileState.error && !profile" key="error" class="card state">
        <h2>{{ errorView.title }}</h2>
        <p>{{ errorView.text }}</p>
        <button class="retry" :disabled="profileState.loading" @click="loadProfile({ force: true })">
          {{ profileState.loading ? 'Пробую...' : 'Попробовать снова' }}
        </button>
        <span v-if="errorView.detail" class="state-detail">{{ errorView.detail }}</span>
      </section>

        <div v-else key="page" class="layout" :class="{ bare: !roomForSkin }">
          <Transition name="panel"><div v-if="roomForSkin" class="skin-panel">
            <div class="skin-stage">
              <Transition name="soft">
                <div v-if="!skinLoaded" class="skin-bone"></div>
              </Transition>
              <SkinViewer3D
                v-if="skinReady"
                class="skin3d"
                :class="{ on: skinLoaded }"
                :url="accountStore.skinUrl"
                :width="240"
                :height="400"
                :lively="false"
                fit
                @ready="skinLoaded = true"
              />
            </div>
            <span class="skin-hint">Потяни, чтобы повернуть</span>
          </div></Transition>

          <div class="side">
          <div class="card identity-body">
            <Transition name="swap" mode="out-in">
              <h2 v-if="profile" key="nick" class="nick">{{ profile.nick }}</h2>
              <span v-else key="bone" class="bone nick-bone"></span>
            </Transition>

            <Transition name="swap" mode="out-in">
              <div v-if="profile" key="pills" class="pills" style="--d: 70ms">
                <span v-if="banText" class="pill banned rise" style="--i: 0" :title="profile.banReason || ''">{{ banText }}</span>
                <span v-if="presence" class="pill rise" :class="{ online: profile.online }" style="--i: 1">{{ presence }}</span>
                <span v-if="profile.playtimeRank" class="pill rank rise" style="--i: 2">#{{ profile.playtimeRank }} по времени в игре</span>
              </div>
              <div v-else key="bone" class="pills"><span class="bone pill-bone"></span></div>
            </Transition>

            <div class="block">
              <span class="label">Роли</span>
              <Transition name="swap" mode="out-in">
                <div v-if="profile" key="roles" class="roles" style="--d: 140ms">
                  <span
                    v-for="(role, i) in profile.roles"
                    :key="role.id"
                    class="role rise"
                    :style="{ background: roleBackground(role.colors), color: roleTextColor(role.colors), '--i': i + 2 }"
                  >
                    <img v-if="role.emoji" class="role-emoji" :src="`${SITE}/api/emoji/${role.emoji}.svg`" alt="" />
                    {{ role.name }}
                  </span>
                  <span v-if="!profile.roles.length" class="muted">Ролей пока нет</span>
                </div>
                <div v-else key="bone" class="roles"><span class="bone role-bone"></span><span class="bone role-bone"></span></div>
              </Transition>
            </div>

            <div class="block uuid-block">
              <div class="uuid-text">
                <span class="label">UUID</span>
                <Transition name="swap" mode="out-in">
                  <code v-if="profile" key="uuid" class="uuid" style="--d: 220ms">{{ profile.uuid }}</code>
                  <span v-else key="bone" class="bone uuid-bone"></span>
                </Transition>
              </div>
              <button
                class="copy-btn"
                :class="{ done: copied }"
                :disabled="!profile"
                :title="copied ? 'Скопировано' : 'Скопировать UUID'"
                :aria-label="copied ? 'UUID скопирован' : 'Скопировать UUID'"
                @click="copyUuid"
              >
                <Transition name="cp" mode="out-in">
                  <svg v-if="copied" key="done" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m5 12.5 4.5 4.5L19 7.5" /></svg>
                  <svg v-else key="copy" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg>
                </Transition>
              </button>
            </div>
          </div>

        <section class="card stats">
          <h3>Статистика</h3>
          <div class="grid">
            <div v-for="(cell, index) in cells" :key="cell.id" class="cell">
              <span class="cell-label">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path :d="cell.icon" /></svg>
                {{ cell.label }}
              </span>
              <Transition name="swap" mode="out-in">
                <span v-if="!profile" key="bone" class="cell-value bone"></span>
                <span v-else-if="stats" key="value" class="cell-value" :style="{ '--d': `${index * 45}ms` }">
                  <CountValue v-if="cell.format" :value="cell.num ?? null" :format="cell.format" :delay="index * 45" />
                  <template v-else>{{ cell.value }}</template>
                </span>
                <span v-else key="none" class="cell-value muted-value">—</span>
              </Transition>
            </div>
          </div>
          <p v-if="statsNote" class="note">{{ statsNote }}</p>
        </section>
          </div>
        </div>
      </Transition>
      </div>
      </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.profile {
  display: flex;
  flex-direction: column;
  gap: 18px;
  height: 100%;
  min-height: 0;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}
.head h1 {
  font-size: 26px;
  font-weight: 800;
  color: var(--text);
}
.refresh {
  display: flex;
  align-items: center;
  gap: 6px;
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 12.5px;
  font-weight: 500;
  cursor: pointer;
  transition: color 0.15s ease;
}
.refresh:hover:not(:disabled) {
  color: var(--text);
}
.refresh:disabled {
  opacity: 0.6;
  cursor: default;
}

.scroll-wrap {
  position: relative;
  flex: 1;
  min-height: 0;
}
.scroll {
  height: 100%;
  overflow-y: auto;
  padding-bottom: 8px;
}
.fit {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.card {
  padding: 22px;
  border-radius: var(--radius-lg);
  background: var(--card);
  border: 1px solid var(--border);
}

/* the figure takes the whole left column, the rest is stacked on the right: it all fits the window without a scroll */
.layout {
  display: flex;
  gap: 16px;
  min-height: 460px;
}
.layout.bare {
  min-height: 0;
}
/* without the figure the statistics go in two tight rows, so the small window still needs no scroll */
.layout.bare .grid {
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}
.layout.bare .cell {
  padding: 10px 10px 10px;
}
.layout.bare .cell-value {
  font-size: 15px;
}
.side {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.skin-panel {
  flex: 0 0 262px;
  box-sizing: border-box;
  min-width: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 10px 12px;
  border-radius: var(--radius-lg);
  background: radial-gradient(ellipse 70% 55% at 50% 58%, rgba(255, 95, 95, 0.22), transparent 70%), var(--surface);
  border: 1px solid var(--border);
}
.skin-stage {
  position: relative;
  flex: 1;
  align-self: stretch;
  min-height: 300px;
  /* the panel grows and shrinks around it, the figure inside keeps its size */
  min-width: 240px;
}
.panel-enter-active,
.panel-leave-active {
  transition: flex-basis 0.38s cubic-bezier(0.2, 0.7, 0.2, 1), margin-right 0.38s cubic-bezier(0.2, 0.7, 0.2, 1),
    padding 0.38s cubic-bezier(0.2, 0.7, 0.2, 1), border-width 0.38s ease, opacity 0.28s ease;
}
.panel-enter-from,
.panel-leave-to {
  flex-basis: 0;
  margin-right: -16px;
  padding-left: 0;
  padding-right: 0;
  border-width: 0;
  opacity: 0;
}
.skin-bone {
  position: absolute;
  inset: 14px 46px 26px;
  border-radius: 18px;
  background: var(--surface-2);
  animation: bone-pulse 1.4s ease-in-out infinite;
}
.skin3d {
  position: absolute;
  inset: 0;
  margin: auto;
  opacity: 0;
  transform: translateY(6px) scale(0.97);
  transition: opacity 0.5s ease, transform 0.6s cubic-bezier(0.2, 0.7, 0.2, 1);
}
.skin3d.on {
  opacity: 1;
  transform: none;
}
.skin-hint {
  font-size: 11.5px;
  color: var(--text-faint);
}

.identity-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px 22px;
}
.nick {
  font-size: 32px;
  font-weight: 800;
  line-height: 1.1;
  color: var(--text);
  overflow-wrap: anywhere;
}

.pills {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  min-height: 28px;
}
.pill {
  padding: 5px 11px;
  border-radius: 8px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 12.5px;
  font-weight: 600;
}
.pill.online {
  background: var(--good-soft);
  color: var(--good);
}
.pill.rank {
  background: rgba(124, 124, 255, 0.16);
  color: #a9a9ff;
}
.pill.banned {
  background: var(--bad-soft);
  color: var(--bad);
}

.block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}
.label {
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-faint);
}

.roles {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.role {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 11px 5px 8px;
  border-radius: 8px;
  font-size: 12.5px;
  font-weight: 700;
}
.role-emoji {
  width: 15px;
  height: 15px;
}
.muted {
  font-size: 13px;
  color: var(--text-faint);
}

.uuid-block {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}
.uuid-text {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.uuid {
  font-family: var(--font-mono);
  font-size: 12.5px;
  color: var(--text-muted);
  overflow-wrap: anywhere;
}
.copy-btn {
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease, transform 0.12s ease;
}
.copy-btn:hover:not(:disabled) {
  background: var(--surface-2);
  color: var(--text);
}
.copy-btn:active:not(:disabled) {
  transform: scale(0.92);
}
.copy-btn.done {
  border-color: var(--good);
  background: var(--good-soft);
  color: var(--good);
  animation: copy-pop 0.4s cubic-bezier(0.2, 0.9, 0.3, 1.3);
}
.cp-enter-active,
.cp-leave-active {
  transition: opacity 0.14s ease, transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.3);
}
.cp-enter-from {
  opacity: 0;
  transform: scale(0.4) rotate(-40deg);
}
.cp-leave-to {
  opacity: 0;
  transform: scale(0.4) rotate(40deg);
}
@keyframes copy-pop {
  0% { box-shadow: 0 0 0 0 rgba(80, 200, 120, 0.45); }
  100% { box-shadow: 0 0 0 10px rgba(80, 200, 120, 0); }
}
.copy-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.stats {
  padding: 18px 22px 20px;
}
.stats h3 {
  font-size: 17px;
  font-weight: 800;
  color: var(--text);
  margin-bottom: 12px;
}
/* always four columns: a count that depends on the width would change the height with the scale */
.grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}
.cell {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 13px 12px 12px;
  border-radius: 12px;
  background: var(--surface);
  border: 1px solid var(--border);
  min-width: 0;
}
.cell-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-muted);
}
.cell-label svg {
  flex-shrink: 0;
  color: var(--text-faint);
}
.cell-value {
  font-size: 16.5px;
  font-weight: 800;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-height: 24px;
}
.muted-value {
  color: var(--text-faint);
}
.note {
  margin-top: 12px;
  font-size: 12.5px;
  color: var(--text-faint);
}

.state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 44px 22px;
  text-align: center;
}
.state h2 {
  font-size: 19px;
  font-weight: 800;
  color: var(--text);
}
.state p {
  max-width: 380px;
  font-size: 13.5px;
  line-height: 1.5;
  color: var(--text-muted);
}
.state-detail {
  max-width: 420px;
  font-size: 11.5px;
  color: var(--text-faint);
  overflow-wrap: anywhere;
}
.retry {
  margin-top: 6px;
  padding: 9px 20px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease, opacity 0.15s ease;
}
.retry:hover:not(:disabled) {
  background: var(--surface);
}
.retry:disabled {
  opacity: 0.6;
  cursor: default;
}

.bone {
  display: block;
  border-radius: 8px;
  background: var(--surface-2);
  animation: bone-pulse 1.4s ease-in-out infinite;
}
.nick-bone {
  width: 220px;
  height: 34px;
}
.pill-bone {
  width: 170px;
  height: 28px;
}
.role-bone {
  width: 96px;
  height: 28px;
}
.uuid-bone {
  width: 290px;
  height: 14px;
}
.cell-value.bone {
  width: 70%;
}
/* data that arrives: the placeholder gives way and the content rises in, each part a little after the previous */
.swap-enter-active {
  transition: opacity 0.4s ease, transform 0.45s cubic-bezier(0.2, 0.7, 0.2, 1);
  transition-delay: var(--d, 0ms);
}
.swap-leave-active {
  transition: opacity 0.12s ease;
}
.swap-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
.swap-leave-to {
  opacity: 0;
}
.soft-leave-active {
  transition: opacity 0.35s ease;
}
.soft-leave-to {
  opacity: 0;
}
.rise {
  animation: rise 0.45s cubic-bezier(0.2, 0.7, 0.2, 1) both;
  animation-delay: calc(var(--i, 0) * 60ms + 80ms);
}
@keyframes rise {
  from { opacity: 0; transform: translateY(6px) scale(0.96); }
  to { opacity: 1; transform: none; }
}
@keyframes bone-pulse {
  0%, 100% { opacity: 0.55; }
  50% { opacity: 1; }
}


@media (prefers-reduced-motion: reduce) {
  .bone,
  .skin-bone,
  .rise {
    animation: none;
  }
  .swap-enter-active,
  .panel-enter-active,
  .panel-leave-active,
  .skin3d,
  .cp-enter-active,
  .cp-leave-active {
    transition: none;
  }
  .copy-btn.done {
    animation: none;
  }
}
</style>
