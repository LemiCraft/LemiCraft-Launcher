<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { accountStore } from '../store/account.js';
import { useLogin } from '../composables/useLogin.js';
import { updateState } from '../store/update.js';
import { violationsState } from '../store/violations.js';
import { rulesState } from '../store/rules.js';
import SkinFace from './SkinFace.vue';

function openUpdateModal() {
  updateState.available = true;
}

const VIOLATION_PRIORITY = { ban: 0, mute: 1, warn: 2 };
const VIOLATION_LABEL = { ban: 'Вы забанены', mute: 'Вы в муте', warn: 'У вас предупреждение' };

const topViolation = computed(() => {
  if (violationsState.items.length === 0) return null;
  return [...violationsState.items].sort((a, b) => VIOLATION_PRIORITY[a.type] - VIOLATION_PRIORITY[b.type])[0];
});
const extraViolationsCount = computed(() => Math.max(0, violationsState.items.length - 1));
const warnCount = computed(() => violationsState.items.filter((v) => v.type === 'warn').length);

// Warns escalate visually with count — yellow → orange → red → burgundy at 4+ (accumulation is itself
// the risk); ban/mute severity is fixed by type, not count
const severity = computed(() => {
  if (!topViolation.value) return null;
  if (topViolation.value.type === 'ban') return 'ban';
  if (topViolation.value.type === 'mute') return 'mute';
  if (warnCount.value >= 4) return 'warn-4';
  return `warn-${warnCount.value}`;
});

// "Навсегда" reads oddly for a warn (it's a standing mark, not a timed restriction) — only ban/mute use it
function violationEndsText(v) {
  if (v.type !== 'warn' && v.permanent) return 'навсегда';
  if (!v.endsAt) return '';
  const mins = Math.round((v.endsAt - Date.now()) / 60000);
  if (mins <= 0) return '';
  const label = v.type === 'warn' ? 'истечёт через' : 'ещё';
  return mins < 60 ? `${label} ${mins} мин` : `${label} ${Math.round(mins / 60)} ч`;
}

function openViolation() {
  if (violationsState.items.length === 0) return;
  // Collect rule numbers cited across ALL active violations, not just the top one — a player
  // can be sitting on several at once, each pointing at a different rule
  const ids = new Set();
  for (const v of violationsState.items) {
    for (const m of v.reason.matchAll(/\d+\.\d+/g)) ids.add(m[0]);
  }
  rulesState.pendingAnchors = [...ids];
  emit('navigate', 'rules');
}

const displayName = computed(() => (accountStore.loggedIn ? accountStore.username : 'Гость'));
const displayRank = computed(() => (accountStore.loggedIn ? accountStore.provider : 'Войти в аккаунт'));

const { loggingInProvider, authError, loginMicrosoft, loginElyBy, cancelLogin, logout } = useLogin();
const menuOpen = ref(false);
const accountWrapEl = ref(null);

function toggleMenu() {
  menuOpen.value = !menuOpen.value;
}

function handleOutsideClick(event) {
  if (menuOpen.value && accountWrapEl.value && !accountWrapEl.value.contains(event.target)) {
    menuOpen.value = false;
  }
}

onMounted(() => document.addEventListener('click', handleOutsideClick));
onUnmounted(() => document.removeEventListener('click', handleOutsideClick));

async function handleLoginMicrosoft() {
  await loginMicrosoft();
  menuOpen.value = false;
}

async function handleLoginElyBy() {
  await loginElyBy();
  menuOpen.value = false;
}

function handleElyByButtonClick() {
  if (loggingInProvider.value === 'elyby') {
    cancelLogin();
  } else {
    handleLoginElyBy();
  }
}

async function handleLogout() {
  await logout();
  menuOpen.value = false;
}

defineProps({
  active: { type: String, required: true },
});
const emit = defineEmits(['navigate']);

const items = [
  { id: 'home', label: 'Главная', icon: 'M4 11.5 12 4l8 7.5V20a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1Z' },
  { id: 'skins', label: 'Скины', icon: 'M12 2 4 6v6c0 5 3.5 8.5 8 10 4.5-1.5 8-5 8-10V6l-8-4Z' },
  { id: 'mods', label: 'Моды', icon: 'M20.5 11H19V7c0-1.1-.9-2-2-2h-4V3.5C13 2.12 11.88 1 10.5 1S8 2.12 8 3.5V5H4c-1.1 0-1.99.9-1.99 2v3.8H3.5c1.49 0 2.7 1.21 2.7 2.7s-1.21 2.7-2.7 2.7H2V20c0 1.1.9 2 2 2h3.8v-1.5c0-1.49 1.21-2.7 2.7-2.7 1.49 0 2.7 1.21 2.7 2.7V22H17c1.1 0 2-.9 2-2v-4h1.5c1.38 0 2.5-1.12 2.5-2.5S21.88 11 20.5 11z' },
  { id: 'settings', label: 'Настройки', icon: 'M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm7.4-3.5c0-.6-.05-1.15-.15-1.7l1.85-1.45-1.85-3.2-2.2.9c-.9-.7-1.9-1.25-3-1.6L13.6 2h-3.2l-.45 2.35c-1.1.35-2.1.9-3 1.6l-2.2-.9-1.85 3.2L4.75 10.3c-.1.55-.15 1.1-.15 1.7s.05 1.15.15 1.7L2.9 15.15l1.85 3.2 2.2-.9c.9.7 1.9 1.25 3 1.6L10.4 22h3.2l.45-2.35c1.1-.35 2.1-.9 3-1.6l2.2.9 1.85-3.2-1.85-1.45c.1-.55.15-1.1.15-1.7Z' },
];

async function openExternal(target) {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('open_external', { target });
}

const quickLinks = [
  { id: 'rules', title: 'Правила сервера', icon: 'M6 2h9l5 5v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Zm8 1.5V8h4.5L14 3.5Z', action: () => emit('navigate', 'rules') },
  { id: 'wiki', title: 'Вики проекта', icon: 'M4 4h16v2H4V4Zm0 5h16v2H4V9Zm0 5h10v2H4v-2Z', action: () => openExternal('https://wiki.lemicraft.ru') },
  { id: 'folder', title: 'Папка игры', icon: 'M4 5h7l2 2h7v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Z', action: async () => {
    if (!window.__TAURI_INTERNALS__) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_game_folder');
  } },
  { id: 'logs', title: 'Логи игры', icon: 'M4 4h16v16H4V4Zm2 3v2h12V7H6Zm0 4v2h12v-2H6Zm0 4v2h8v-2H6Z', action: async () => {
    if (!window.__TAURI_INTERNALS__) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_logs_window');
  } },
];
</script>

<template>
  <aside class="sidebar" data-tauri-drag-region>
    <div class="brand">
      <img class="brand-mark" src="../assets/logo.png" alt="LemiCraft" />
      <span class="brand-name">Lemi<b>Craft</b></span>
    </div>

    <nav class="nav">
      <button
        v-for="item in items"
        :key="item.id"
        class="nav-item"
        :class="{ active: active === item.id }"
        @click="emit('navigate', item.id)"
      >
        <svg viewBox="0 0 24 24" width="19" height="19" fill="currentColor"><path :d="item.icon" /></svg>
        <span>{{ item.label }}</span>
      </button>
    </nav>

    <div v-if="updateState.info" class="update-banner">
      <span class="update-banner-text">Доступна версия {{ updateState.info.version }}</span>
      <button class="update-banner-btn" @click="openUpdateModal">Открыть</button>
    </div>

    <Transition name="violation-fade">
    <button v-if="topViolation" class="violation-banner" :class="severity" @click="openViolation">
      <span class="violation-banner-text">
        {{ VIOLATION_LABEL[topViolation.type] }}
        <template v-if="violationEndsText(topViolation)"> · {{ violationEndsText(topViolation) }}</template>
        <template v-if="extraViolationsCount > 0"> · ещё {{ extraViolationsCount }}</template>
      </span>
      <span class="violation-banner-reason">{{ topViolation.reason }}</span>
      <span v-if="warnCount >= 2" class="violation-banner-hint">Накопление предупреждений может привести к бану</span>
    </button>
    </Transition>

    <div class="quick-links">
      <button v-for="link in quickLinks" :key="link.id" class="quick-link" :aria-label="link.title" @click="link.action">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path :d="link.icon" /></svg>
        <span class="tip">{{ link.title }}</span>
      </button>
    </div>

    <div class="account-wrap" ref="accountWrapEl">
      <Transition name="menu">
        <div v-if="menuOpen" class="account-menu">
          <template v-if="accountStore.loggedIn">
            <div class="menu-info">
              <span class="menu-name">{{ accountStore.username }}</span>
              <span class="menu-sub">{{ accountStore.provider }}-аккаунт</span>
            </div>
            <button class="menu-item danger" @click="handleLogout">Выйти</button>
          </template>
          <template v-else>
            <button class="menu-item" :disabled="!!loggingInProvider" @click="handleLoginMicrosoft">
              {{ loggingInProvider === 'microsoft' ? 'Открываю окно входа...' : 'Войти через Microsoft' }}
            </button>
            <button
              class="menu-item"
              :class="{ cancellable: loggingInProvider === 'elyby' }"
              :disabled="loggingInProvider === 'microsoft'"
              @click="handleElyByButtonClick"
            >
              {{ loggingInProvider === 'elyby' ? 'Отменить вход через ely.by' : 'Войти через Ely.by' }}
            </button>
            <div class="menu-register">
              <span @click="openExternal('https://account.ely.by/register')">Нет аккаунта Ely.by?</span>
              <span @click="openExternal('https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc')">Купить Minecraft</span>
            </div>
            <p v-if="authError" class="menu-error">{{ authError }}</p>
          </template>
        </div>
      </Transition>

      <button class="account" @click="toggleMenu">
        <SkinFace v-if="accountStore.loggedIn && accountStore.skinUrl" :url="accountStore.skinUrl" :size="32" :radius="9" />
        <div v-else class="avatar">
          {{ displayName.charAt(0) }}
        </div>
        <div class="account-info">
          <span class="account-name">{{ displayName }}</span>
          <span class="account-rank">{{ displayRank }}</span>
        </div>
        <svg class="chevron" :class="{ open: menuOpen }" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
          <path d="M7 10l5 5 5-5H7Z" />
        </svg>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  -webkit-app-region: drag;
  width: 232px;
  flex-shrink: 0;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: 20px 14px;
  gap: 8px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px 22px;
}

.brand-mark {
  width: 32px;
  height: 32px;
  border-radius: 10px;
  object-fit: cover;
  flex-shrink: 0;
  box-shadow: 0 6px 16px -6px rgba(255, 95, 95, 0.55);
}

.brand-name {
  font-family: var(--font-display);
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
  letter-spacing: -0.01em;
}
.brand-name b {
  font-weight: 800;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
}

.nav-item {
  -webkit-app-region: no-drag;
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px 10px 15px;
  border-radius: var(--radius-sm);
  background: transparent;
  border: none;
  border-left: 3px solid transparent;
  color: var(--text-muted);
  font-size: 14.5px;
  font-weight: 500;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s ease, color 0.15s ease, border-color 0.2s ease;
}

.nav-item:hover {
  background: var(--surface);
  color: var(--text);
}

.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent);
  border-left-color: var(--accent);
}

.nav-item svg {
  transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  flex-shrink: 0;
}
.nav-item:active svg {
  transform: scale(0.85);
}

.violation-banner {
  -webkit-app-region: no-drag;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  margin: 4px 0 0;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid transparent;
  background: none;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s ease;
  width: 100%;
}

.violation-fade-enter-active,
.violation-fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease, margin-top 0.2s ease;
}
.violation-fade-enter-from,
.violation-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
.violation-banner:hover {
  background: var(--surface);
}
.violation-banner.mute,
.violation-banner.warn-1 {
  border-color: rgba(231, 200, 115, 0.4);
  background: var(--gold-soft);
}
.violation-banner.warn-2 {
  border-color: rgba(240, 136, 62, 0.4);
  background: rgba(240, 136, 62, 0.14);
}
.violation-banner.warn-3,
.violation-banner.ban {
  border-color: rgba(239, 68, 68, 0.35);
  background: var(--bad-soft);
}
.violation-banner.warn-4 {
  border-color: rgba(161, 35, 49, 0.55);
  background: rgba(161, 35, 49, 0.18);
}
.violation-banner-text {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
}
.violation-banner-reason {
  font-size: 11.5px;
  color: var(--text-muted);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  max-width: 100%;
}
.violation-banner-hint {
  font-size: 10.5px;
  color: var(--bad);
  font-weight: 600;
  max-width: 100%;
}

.update-banner {
  -webkit-app-region: no-drag;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 4px 0 0;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(255, 95, 95, 0.35);
  background: var(--accent-soft);
}
.update-banner-text {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
}
.update-banner-btn {
  -webkit-app-region: no-drag;
  width: 100%;
  padding: 8px 0;
  border-radius: 7px;
  border: none;
  background: var(--accent);
  color: var(--accent-text-on);
  font-size: 12.5px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.15s ease;
}
.update-banner-btn:hover {
  background: var(--accent-hover);
}

.quick-links {
  display: flex;
  gap: 6px;
  padding: 4px 12px 12px;
}

.quick-link {
  -webkit-app-region: no-drag;
  position: relative;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-faint);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease, transform 0.1s ease;
}
.quick-link:hover {
  background: var(--surface-hover);
  color: var(--text);
  transform: translateY(-1px);
}

.tip {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 50%;
  transform: translate(-50%, 4px);
  white-space: nowrap;
  background: var(--surface-2);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 7px;
  padding: 5px 9px;
  font-size: 11.5px;
  font-weight: 600;
  box-shadow: var(--shadow-pop);
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease, transform 0.12s ease;
  z-index: 30;
}
.quick-link:hover .tip {
  opacity: 1;
  transform: translate(-50%, 0);
}
.quick-link:first-child .tip {
  left: 0;
  transform: translate(0, 4px);
}
.quick-link:first-child:hover .tip {
  transform: translate(0, 0);
}
.quick-link:last-child .tip {
  left: auto;
  right: 0;
  transform: translate(0, 4px);
}
.quick-link:last-child:hover .tip {
  transform: translate(0, 0);
}

.account-wrap {
  position: relative;
}

.account {
  -webkit-app-region: no-drag;
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border-radius: var(--radius-sm);
  background: var(--surface);
  border: 1px solid var(--border);
  cursor: pointer;
  transition: background 0.15s ease;
}
.account:hover {
  background: var(--surface-hover);
}

.avatar {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--surface-2);
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 14px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.account-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
  align-items: flex-start;
  flex: 1;
}

.account-name {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 120px;
}

.account-rank {
  font-size: 11.5px;
  color: var(--gold);
}

.chevron {
  flex-shrink: 0;
  color: var(--text-faint);
  transition: transform 0.15s ease;
}
.chevron.open {
  transform: rotate(180deg);
}

.account-menu {
  -webkit-app-region: no-drag;
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  right: 0;
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-pop);
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  z-index: 20;
}

.menu-info {
  padding: 6px 8px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 4px;
}
.menu-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.menu-sub {
  font-size: 11px;
  color: var(--text-faint);
}

.menu-item {
  text-align: left;
  padding: 8px;
  border-radius: 7px;
  border: none;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.menu-item:hover:not(:disabled) {
  background: var(--surface-hover);
}
.menu-item:disabled {
  opacity: 0.6;
  cursor: default;
}
.menu-item.cancellable {
  color: var(--text-muted);
}
.menu-item.cancellable:hover {
  background: var(--bad-soft);
  color: var(--bad);
}
.menu-item.danger {
  color: var(--bad);
}
.menu-item.danger:hover {
  background: var(--bad-soft);
}

.menu-error {
  padding: 6px 8px 0;
  font-size: 11.5px;
  color: var(--bad);
}

.menu-register {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 8px 2px;
}
.menu-register span {
  font-size: 11px;
  color: var(--text-faint);
  cursor: pointer;
  transition: color 0.15s ease;
}
.menu-register span:hover {
  color: var(--accent);
}

.menu-enter-active,
.menu-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.menu-enter-from,
.menu-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
