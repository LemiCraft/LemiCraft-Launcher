<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { accountStore } from '../store/account.js';
import { useLogin } from '../composables/useLogin.js';
import { updateState } from '../store/update.js';
import { violationsState } from '../store/violations.js';
import { rulesState } from '../store/rules.js';
import SkinFace from './SkinFace.vue';
import MacTrafficLights from './MacTrafficLights.vue';
import { isMac } from '../utils/platform.js';
import rulesIcon from '../assets/rules.svg?raw';
import helpIcon from '../assets/help.svg?raw';
import folderIcon from '../assets/folder.svg?raw';
import gamepadIcon from '../assets/gamepad.svg?raw';

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

// Warns escalate with count (yellow → orange → red → burgundy at 4+); ban/mute severity is fixed by type
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
  // Rule numbers from ALL active violations, not just the top one: a player can hold several, each citing a different rule
  const ids = new Set();
  for (const v of violationsState.items) {
    for (const m of v.reason.matchAll(/\d+\.\d+/g)) ids.add(m[0]);
  }
  rulesState.pendingAnchors = [...ids];
  emit('navigate', 'rules');
}

const displayName = computed(() => (accountStore.loggedIn ? accountStore.username : 'Гость'));
const displayRank = computed(() => (accountStore.loggedIn ? accountStore.provider : 'Войти в аккаунт'));

const { authError, loginMicrosoft, loginElyBy, logout } = useLogin();
const menuOpen = ref(false);
const accountWrapEl = ref(null);

function toggleMenu() {
  menuOpen.value = !menuOpen.value;
}

function activateQuickLink(link) {
  link.action();
  menuOpen.value = false;
}

function openPersonalAccount() {
  const target = accountStore.provider === 'Ely.by'
    ? 'https://account.ely.by/'
    : 'https://www.minecraft.net/msaprofile';
  openExternal(target);
  menuOpen.value = false;
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

async function handleLogout() {
  await logout();
  menuOpen.value = false;
}

defineProps({
  active: { type: String, required: true },
  maximized: { type: Boolean, default: false },
});
const emit = defineEmits(['navigate', 'window-close', 'window-minimize', 'window-maximize']);

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
  { id: 'rules', title: 'Правила сервера', icon: rulesIcon, action: () => emit('navigate', 'rules') },
  { id: 'wiki', title: 'Помощь по серверу', icon: helpIcon, action: () => openExternal('https://wiki.lemicraft.ru') },
  { id: 'folder', title: 'Папка с игрой', icon: folderIcon, action: async () => {
    if (!window.__TAURI_INTERNALS__) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_game_folder');
  } },
  { id: 'logs', title: 'Логи игры', icon: gamepadIcon, action: async () => {
    if (!window.__TAURI_INTERNALS__) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_logs_window');
  } },
];

</script>

<template>
  <aside class="sidebar" data-tauri-drag-region>
    <div v-if="isMac" class="mac-lights-row" data-tauri-drag-region>
      <MacTrafficLights
        :maximized="maximized"
        @close="emit('window-close')"
        @minimize="emit('window-minimize')"
        @maximize="emit('window-maximize')"
      />
    </div>

    <div class="brand" data-tauri-drag-region>
      <img class="brand-mark" src="../assets/logo.png" alt="LemiCraft" />
      <span class="brand-name" data-tauri-drag-region>Lemi<b>Craft</b></span>
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

    <div class="account-wrap" ref="accountWrapEl">
      <Transition name="menu">
        <div v-if="menuOpen" class="account-menu">
          <div v-if="accountStore.loggedIn" class="profile-head">
            <div class="profile-details">
              <span class="profile-name">{{ displayName }}</span>
              <span class="profile-provider">{{ displayRank }}</span>
            </div>
            <SkinFace
              v-if="accountStore.skinUrl"
              class="profile-skin"
              :url="accountStore.skinUrl"
              :size="70"
              :radius="10"
            />
            <div v-else class="profile-avatar">{{ displayName.charAt(0) }}</div>
          </div>

          <button v-if="accountStore.loggedIn" class="menu-account-link" @click="openPersonalAccount">
            Личный кабинет
          </button>

          <div v-else class="guest-login">
            <div class="guest-heading">
              <div class="profile-details">
                <span class="profile-name">Гость</span>
                <span class="profile-provider">Войдите в аккаунт</span>
              </div>
              <div class="profile-avatar">Г</div>
            </div>
            <button class="menu-item" @click="handleLoginMicrosoft">Войти через Microsoft</button>
            <button class="menu-item" @click="handleLoginElyBy">Войти через Ely.by</button>
          </div>

          <div class="quick-links-grid">
            <button
              v-for="link in quickLinks"
              :key="link.id"
              class="quick-link-card"
              :class="{ active: link.id === active }"
              :aria-current="link.id === active ? 'page' : undefined"
              @click="activateQuickLink(link)"
            >
              <span class="quick-link-icon" aria-hidden="true" v-html="link.icon" />
              <span>{{ link.title }}</span>
            </button>
          </div>

          <div v-if="!accountStore.loggedIn" class="menu-register">
            <span @click="openExternal('https://account.ely.by/register')">Нет аккаунта Ely.by?</span>
            <span @click="openExternal('https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc')">Купить Minecraft</span>
          </div>
          <p v-if="authError" class="menu-error">{{ authError }}</p>
          <button v-if="accountStore.loggedIn" class="menu-logout" @click="handleLogout">Выйти</button>
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
  width: 236px;
  flex-shrink: 0;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: 20px 14px;
  gap: 8px;
}

/* The traffic lights row sits above the brand, so the top padding shrinks */
:root.mac .sidebar {
  padding-top: 14px;
}

/* Above the login overlay (z-index 120), like the Windows window controls */
.mac-lights-row {
  position: relative;
  z-index: 130;
  padding: 2px 6px 12px;
  -webkit-app-region: drag;
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

.account-wrap {
  position: relative;
  margin-right: -5px;
}

.account {
  -webkit-app-region: no-drag;
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: 18px;
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
  font-size: 16px;
  line-height: 1.05;
  font-weight: 700;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 120px;
}

.account-rank {
  font-size: 14px;
  line-height: 1.05;
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
  bottom: calc(100% + 10px);
  left: 0;
  width: 212px;
  background: #1d1c22;
  border: 1px solid var(--border);
  border-radius: 20px;
  box-shadow: var(--shadow-pop);
  padding: 20px 8px 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: 40;
}

.profile-head,
.guest-heading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  min-width: 0;
}
.profile-skin {
  flex-shrink: 0;
  margin-top: 2px;
}
.profile-avatar {
  width: 54px;
  height: 54px;
  flex-shrink: 0;
  display: grid;
  place-items: center;
  border-radius: 14px;
  background: var(--surface);
  color: var(--text-muted);
  font-size: 20px;
  font-weight: 700;
}
.profile-details {
  width: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
}
.profile-name {
  max-width: 100%;
  color: var(--text);
  font-size: 16px;
  line-height: 1.05;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.profile-provider {
  color: var(--gold);
  font-size: 14px;
  line-height: 1.05;
}
.menu-account-link {
  margin-top: 8px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  padding: 0 8px;
  border: 1px solid #3a3032;
  border-radius: 9px;
  background: #bd0000;
  color: #fff;
  font: 700 12px var(--font-body);
  cursor: pointer;
  transition: background 0.15s ease;
}
.menu-account-link:hover {
  background: #d00000;
}
.guest-login {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.guest-heading {
  margin-bottom: 6px;
}
.quick-links-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 4px;
}
.quick-link-card {
  -webkit-app-region: no-drag;
  min-width: 0;
  height: 36px;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 2px;
  padding: 2px 4px 2px 7px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  text-align: center;
  font: 700 10px/1.1 var(--font-body);
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}
.quick-link-card:hover {
  background: var(--surface-hover);
  border-color: var(--text-faint);
}
.quick-link-card.active {
  color: var(--accent);
  border-color: var(--accent);
  background: var(--accent-soft);
}
.quick-link-card > span:last-child {
  flex: 1;
  min-width: 0;
}
.quick-link-card:nth-child(odd) {
  flex-direction: row-reverse;
  padding-left: 4px;
  padding-right: 4px;
}
.quick-link-icon {
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #5b5662;
}
.quick-link-icon :deep(svg) {
  display: block;
  width: 100%;
  height: 100%;
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
.menu-item:hover {
  background: var(--surface-hover);
}
.menu-item.danger {
  color: var(--bad);
}
.menu-item.danger:hover {
  background: var(--bad-soft);
}

.menu-logout {
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--bad);
  font: 500 12px var(--font-body);
  cursor: pointer;
  transition: background 0.15s ease;
}
.menu-logout:hover {
  background: var(--bad-soft);
}

.menu-error {
  padding: 6px 8px 0;
  font-size: 11.5px;
  color: var(--bad);
  overflow-wrap: anywhere;
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
  transform-origin: bottom left;
  transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.menu-enter-from,
.menu-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.96);
}
</style>
