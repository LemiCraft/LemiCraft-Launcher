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
import ghostIcon from '../assets/ghost.svg?raw';
import profileRulesIcon from '../assets/profile-rules.svg?raw';
import profileWikiIcon from '../assets/profile-wiki.svg?raw';
import profileFolderIcon from '../assets/profile-folder.svg?raw';
import profileLogsIcon from '../assets/profile-logs.svg?raw';
import elyByFavicon from '../assets/elyby-favicon.ico';

function tintIcon(svg) {
  return svg
    .replace(/<\?xml[^>]*\?>/gi, '')
    .replace(/<!DOCTYPE[\s\S]*?>/gi, '')
    .replace(/<!--[^]*?-->/g, '')
    .replace(/<style[^>]*>([^]*?)<\/style>/gi, (_, rules) => `<style>${rules.replace(/#[\da-f]{3,8}/gi, 'currentColor')}</style>`)
    .replace(/(fill|stroke)="(?:#[\da-f]{3,8}|black|white)"/gi, '$1="currentColor"')
    .replace(/(<rect[^>]*?)fill="currentColor"/gi, '$1fill="none"');
}

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
const loginChoiceOpen = ref(false);
const accountWrapEl = ref(null);

function toggleMenu() {
  menuOpen.value = !menuOpen.value;
}

function openLoginChoice() {
  menuOpen.value = false;
  loginChoiceOpen.value = true;
}

function activateQuickLink(link) {
  link.action();
  menuOpen.value = false;
}

function openPersonalAccount() {
  emit('navigate', 'profile');
  menuOpen.value = false;
}

function handleOutsideClick(event) {
  if (menuOpen.value && accountWrapEl.value && !accountWrapEl.value.contains(event.target)) {
    menuOpen.value = false;
  }
}

function handleLoginChoiceKeydown(event) {
  if (event.key === 'Escape') loginChoiceOpen.value = false;
}

onMounted(() => {
  document.addEventListener('click', handleOutsideClick);
  document.addEventListener('keydown', handleLoginChoiceKeydown);
});
onUnmounted(() => {
  document.removeEventListener('click', handleOutsideClick);
  document.removeEventListener('keydown', handleLoginChoiceKeydown);
});

async function handleLoginMicrosoft() {
  loginChoiceOpen.value = false;
  await loginMicrosoft();
  menuOpen.value = false;
}

async function handleLoginElyBy() {
  loginChoiceOpen.value = false;
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
  { id: 'rules', title: 'Правила', icon: profileRulesIcon, action: () => emit('navigate', 'rules') },
  { id: 'wiki', title: 'Вики', icon: profileWikiIcon, action: () => openExternal('https://wiki.lemicraft.ru') },
  { id: 'folder', title: 'Папка игры', icon: profileFolderIcon, action: async () => {
    if (!window.__TAURI_INTERNALS__) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_game_folder');
  } },
  { id: 'logs', title: 'Логи', icon: profileLogsIcon, action: async () => {
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
              <span class="profile-provider">
                <svg v-if="accountStore.provider === 'Microsoft'" class="provider-microsoft-mark" viewBox="0 0 24 24" aria-hidden="true">
                  <path fill="#f25022" d="M1 1h10v10H1z" />
                  <path fill="#7fba00" d="M13 1h10v10H13z" />
                  <path fill="#00a4ef" d="M1 13h10v10H1z" />
                  <path fill="#ffb900" d="M13 13h10v10H13z" />
                </svg>
                <img v-else-if="accountStore.provider === 'Ely.by'" class="provider-elyby-mark" :src="elyByFavicon" alt="" />
                {{ displayRank }}
              </span>
            </div>
            <SkinFace
              v-if="accountStore.skinUrl"
              class="profile-skin"
              :url="accountStore.skinUrl"
              :size="70"
              :radius="10"
            />
            <div v-else-if="accountStore.skinLoading" class="profile-avatar skeleton"></div>
            <div v-else class="profile-avatar">{{ displayName.charAt(0) }}</div>
          </div>

          <button v-if="accountStore.loggedIn" class="menu-account-link" @click="openPersonalAccount">
            Личный кабинет
          </button>

          <div v-else class="guest-login">
            <div class="guest-heading">
              <div class="profile-details">
                <span class="profile-name">{{ accountStore.checking ? 'Проверяю аккаунт' : 'Гость' }}</span>
                <span class="profile-provider">{{ accountStore.checking ? 'Подождите секунду' : 'Войдите в аккаунт' }}</span>
              </div>
              <div v-if="accountStore.checking" class="profile-avatar skeleton"></div>
              <div v-else class="profile-avatar guest-avatar" v-html="tintIcon(ghostIcon)"></div>
            </div>
            <button class="menu-account-link guest-login-button" :disabled="accountStore.checking" @click="openLoginChoice">
              {{ accountStore.checking ? 'Проверяю...' : 'Войти' }}
            </button>
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
              <span class="quick-link-icon" aria-hidden="true" v-html="tintIcon(link.icon)" />
              <span>{{ link.title }}</span>
            </button>
          </div>

          <p v-if="authError" class="menu-error">{{ authError }}</p>
          <button v-if="accountStore.loggedIn" class="menu-logout" @click="handleLogout">Выйти</button>
        </div>
      </Transition>

      <Transition name="login-choice">
      <div
        v-if="loginChoiceOpen"
        class="login-choice-overlay"
        role="presentation"
        @click.self="loginChoiceOpen = false"
      >
          <section class="login-choice-dialog" role="dialog" aria-modal="true" aria-labelledby="login-choice-title">
            <button class="login-choice-close" aria-label="Закрыть" @click="loginChoiceOpen = false">×</button>
            <h2 id="login-choice-title">Войти в аккаунт</h2>
            <p>Выберите способ входа</p>
            <button class="provider-choice" @click="handleLoginMicrosoft">
              <svg class="microsoft-logo" viewBox="0 0 24 24" aria-hidden="true">
                <path fill="#f25022" d="M1 1h10v10H1z" />
                <path fill="#7fba00" d="M13 1h10v10H13z" />
                <path fill="#00a4ef" d="M1 13h10v10H1z" />
                <path fill="#ffb900" d="M13 13h10v10H13z" />
              </svg>
              <span>Продолжить с Microsoft</span>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 18 6-6-6-6" /></svg>
            </button>
            <button class="provider-choice" @click="handleLoginElyBy">
              <img class="provider-dialog-elyby-mark" :src="elyByFavicon" alt="" />
              <span>Продолжить с Ely.by</span>
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 18 6-6-6-6" /></svg>
            </button>
            <div class="login-choice-links">
              <button @click="openExternal('https://account.ely.by/register')">Нет аккаунта Ely.by?</button>
              <button @click="openExternal('https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc')">Купить Minecraft</button>
            </div>
          </section>
        </div>
      </Transition>

      <button class="account" @click="toggleMenu">
        <!-- the old and the new picture and text sit in the same place while they cross-fade, so nothing around them moves -->
        <div class="account-avatar-slot">
          <Transition name="account-fade">
            <div v-if="accountStore.checking" key="checking" class="avatar skeleton account-layer"></div>
            <SkinFace v-else-if="accountStore.loggedIn && accountStore.skinUrl" :key="accountStore.skinUrl" class="account-layer" :url="accountStore.skinUrl" :size="32" :radius="9" />
            <div v-else-if="accountStore.loggedIn && accountStore.skinLoading" key="skeleton" class="avatar skeleton account-layer"></div>
            <div v-else-if="!accountStore.loggedIn" key="guest" class="avatar guest-avatar account-layer" v-html="tintIcon(ghostIcon)"></div>
            <div v-else key="letter" class="avatar account-layer">
              {{ displayName.charAt(0) }}
            </div>
          </Transition>
        </div>
        <div class="account-info">
          <span class="account-text-slot">
            <Transition name="account-fade">
              <span v-if="accountStore.checking" key="checking" class="account-name text-bone"></span>
              <span v-else :key="displayName" class="account-name">{{ displayName }}</span>
            </Transition>
          </span>
          <span class="account-text-slot">
            <Transition name="account-fade">
              <span v-if="accountStore.checking" key="checking" class="account-rank text-bone short"></span>
              <span v-else :key="displayRank" class="account-rank">{{ displayRank }}</span>
            </Transition>
          </span>
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

.account-avatar-slot {
  position: relative;
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
}
.account-layer {
  position: absolute;
  top: 0;
  left: 0;
}
.account-text-slot {
  display: grid;
  min-width: 0;
  max-width: 100%;
}
.account-text-slot > * {
  grid-area: 1 / 1;
  min-width: 0;
}
.text-bone {
  display: block;
  width: 92px;
  height: 14px;
  margin: 1px 0;
  border-radius: 6px;
  background: var(--surface-2);
  animation: skeleton-pulse 1.4s ease-in-out infinite;
}
.text-bone.short {
  width: 58px;
  height: 12px;
}
.account-fade-enter-active,
.account-fade-leave-active {
  transition: opacity 0.25s ease;
}
.account-fade-enter-from,
.account-fade-leave-to {
  opacity: 0;
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
  color: var(--text-muted);
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
  border-radius: 24px;
  box-shadow: var(--shadow-pop);
  padding: 14px 8px 8px;
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
  gap: 5px;
  min-width: 0;
}
.profile-skin {
  flex-shrink: 0;
  margin-top: 2px;
  outline: 2px solid #49444f;
  outline-offset: 2px;
}
.skeleton {
  color: transparent;
  animation: skeleton-pulse 1.4s ease-in-out infinite;
}
@keyframes skeleton-pulse {
  0%, 100% { opacity: 0.55; }
  50% { opacity: 1; }
}
.profile-avatar {
  width: 70px;
  height: 70px;
  flex-shrink: 0;
  display: grid;
  place-items: center;
  border-radius: 12px;
  background: #111013;
  border: 2px solid #49444f;
  color: var(--text-muted);
  font-size: 26px;
  font-weight: 700;
}
.profile-avatar :deep(svg),
.avatar.guest-avatar :deep(svg) {
  display: block;
  width: 62%;
  height: 62%;
}
.avatar.guest-avatar :deep(svg) {
  width: 19px;
  height: 19px;
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
  font-size: 17px;
  line-height: 1.05;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.profile-provider {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--text-muted);
  font-size: 14px;
  line-height: 1.05;
}
.provider-microsoft-mark {
  width: 13px;
  height: 13px;
  flex: 0 0 13px;
}
.provider-elyby-mark {
  width: 13px;
  height: 13px;
  flex: 0 0 13px;
  border-radius: 3.5px;
  image-rendering: pixelated;
}
.menu-account-link {
  margin-top: 2px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  padding: 0 8px;
  border: 1px solid #59383c;
  border-radius: 14px;
  background: #642e33;
  color: #ffa2a8;
  font: 700 14px var(--font-body);
  cursor: pointer;
  transition: background 0.15s ease;
}
.menu-account-link:hover {
  background: #76373d;
}
.guest-login-button {
  margin-top: 0;
  background: transparent;
  border-color: var(--border);
  color: var(--text);
}
.guest-login-button:hover {
  background: var(--surface-hover);
}
.guest-login {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.guest-heading {
  margin-bottom: 4px;
}
.provider-mark {
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  display: grid;
  place-items: center;
  border-radius: 5px;
}
.quick-links-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 7px 5px;
}
.quick-link-card {
  -webkit-app-region: no-drag;
  min-width: 0;
  height: 46px;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 5px;
  padding: 3px 8px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: transparent;
  color: var(--text);
  text-align: center;
  font: 700 11px/1.1 var(--font-body);
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
  flex-direction: row;
}
.quick-link-icon {
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #77717f;
}
.quick-link-icon :deep(svg) {
  display: block;
  width: 100%;
  height: 100%;
}
.quick-link-icon :deep(path),
.quick-link-icon :deep(circle),
.quick-link-icon :deep(g) {
  stroke: currentColor;
}
.login-choice-overlay {
  -webkit-app-region: no-drag;
  position: fixed;
  inset: 0;
  z-index: 180;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(8, 7, 9, 0.78);
}
.login-choice-enter-active,
.login-choice-leave-active {
  transition: opacity 0.2s ease;
}
.login-choice-enter-active .login-choice-dialog,
.login-choice-leave-active .login-choice-dialog {
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1), opacity 0.2s ease;
}
.login-choice-enter-from,
.login-choice-leave-to {
  opacity: 0;
}
.login-choice-enter-from .login-choice-dialog,
.login-choice-leave-to .login-choice-dialog {
  opacity: 0;
  transform: translateY(14px) scale(0.97);
}
.login-choice-dialog {
  position: relative;
  width: min(360px, 100%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 28px 24px 24px;
  border: 1px solid var(--border);
  border-radius: 24px;
  background: var(--surface);
  box-shadow: var(--shadow-pop);
}
.login-choice-close {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 30px;
  height: 30px;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--text-faint);
  font: 400 24px/1 var(--font-body);
  cursor: pointer;
}
.login-choice-close:hover { background: var(--surface-hover); color: var(--text); }
.login-choice-dialog h2 {
  margin-top: 4px;
  color: var(--text);
  font: 800 19px/1.2 var(--font-display);
}
.login-choice-dialog > p { margin: -3px 0 8px; color: var(--text-muted); font-size: 13px; }
.provider-choice {
  width: 100%;
  min-height: 50px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: 13px;
  background: transparent;
  color: var(--text);
  font: 650 13px var(--font-body);
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, transform 0.15s ease;
}
.provider-choice:hover { background: var(--surface-hover); border-color: var(--text-faint); transform: translateY(-1px); }
.provider-choice > span:nth-child(2) { flex: 1; }
.provider-choice > svg:last-child { width: 16px; height: 16px; fill: none; stroke: var(--text-faint); stroke-width: 1.8; }
.provider-choice > .microsoft-logo { width: 22px; height: 22px; flex: 0 0 22px; }
.provider-dialog-elyby-mark { width: 22px; height: 22px; flex: 0 0 22px; border-radius: 6px; image-rendering: pixelated; }
.login-choice-links {
  width: 100%;
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-top: 4px;
}
.login-choice-links button {
  max-width: 50%;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-faint);
  font: 12px/1.35 var(--font-body);
  text-align: left;
  cursor: pointer;
  transition: color 0.15s ease;
}
.login-choice-links button:hover { color: var(--text); }
.menu-logout {
  height: 30px;
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

/* the menu sits right above its button, so it only rises: no scaling, which looked like it grew out of the left corner */
.menu-enter-active,
.menu-leave-active {
  transition: opacity 0.2s ease, transform 0.28s cubic-bezier(0.16, 1, 0.3, 1);
  will-change: opacity, transform;
}
.menu-enter-from,
.menu-leave-to {
  opacity: 0;
  transform: translateY(12px);
}
@media (prefers-reduced-motion: reduce) {
  .menu-enter-active,
  .menu-leave-active,
  .login-choice-enter-active,
  .login-choice-leave-active,
  .login-choice-enter-active .login-choice-dialog,
  .login-choice-leave-active .login-choice-dialog,
  .account-fade-enter-active,
  .account-fade-leave-active {
    transition-duration: 0.01ms;
  }
}
</style>
