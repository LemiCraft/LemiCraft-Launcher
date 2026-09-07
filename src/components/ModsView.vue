<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import {
  modsState,
  loadModCatalog,
  loadOfficialPack,
  applyOfficialPack,
  uninstallOfficialPack,
  installModIds,
  uninstallModId,
  previewImportCode,
  applyImportCode,
  isPackOnly,
} from '../store/mods.js';
import { formatFileSize } from '../utils/format.js';
import { pushNotification } from '../store/notifications.js';
import { showConfirmDialog } from '../store/confirmDialog.js';

const CATEGORIES = [
  { id: 'all', label: 'Все' },
  { id: 'performance', label: 'Оптимизация' },
  { id: 'visual', label: 'Визуал' },
  { id: 'interface', label: 'Интерфейс' },
  { id: 'social', label: 'Социальное' },
];

const activeCategory = ref('all');
const refreshing = ref(false);

const scrollEl = ref(null);
const topFaded = ref(false);
const bottomFaded = ref(false);
function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  topFaded.value = el.scrollTop > 4;
  bottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 4;
}

const byId = computed(() => Object.fromEntries(modsState.catalog.map((m) => [m.id, m])));

const visibleCatalog = computed(() => modsState.catalog.filter((m) => !m.hidden && m.category !== 'library'));

const filteredCatalog = computed(() =>
  activeCategory.value === 'all' ? visibleCatalog.value : visibleCatalog.value.filter((m) => m.category === activeCategory.value),
);

function isInstalled(entry) {
  return entry.id in modsState.installed;
}

function conflictingNames(entry) {
  return entry.conflicts.filter((id) => id in modsState.installed).map((id) => byId.value[id]?.name || id);
}

function iconUrl(entry) {
  return `https://lemicraft.ru/api/mod-icon/${entry.modrinthId}`;
}

function modrinthUrl(entry) {
  return `https://modrinth.com/mod/${entry.modrinthId}`;
}

function hideBrokenIcon(event) {
  event.target.style.display = 'none';
}

async function openExternal(url) {
  if (!window.__TAURI_INTERNALS__) return;
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('open_external', { target: url });
}

async function onToggleCard(entry) {
  if (modsState.applying) return;
  if (!isInstalled(entry) && conflictingNames(entry).length) return;

  if (isInstalled(entry) && isPackOnly(entry.id)) {
    pushNotification('Этот мод входит в сборку LemiSborka — уберите всю сборку целиком в её карточке выше', 'error');
    return;
  }

  try {
    if (isInstalled(entry)) {
      await uninstallModId(entry.id);
      pushNotification(`Мод «${entry.name}» удалён`);
    } else {
      await installModIds([entry.id]);
      pushNotification(`Мод «${entry.name}» установлен`);
    }
  } catch (err) {
    pushNotification(String(err), 'error');
  }
}

// Home banner sets this flag but doesn't own its lifetime — clear it here after one pulse
watch(
  () => modsState.highlightPackInstall,
  (on) => {
    if (!on) return;
    setTimeout(() => (modsState.highlightPackInstall = false), 1800);
  },
);

async function onApplyOfficialPack() {
  try {
    await applyOfficialPack();
    pushNotification(`Сборка «${modsState.officialPack?.name}» установлена`);
  } catch (err) {
    pushNotification(String(err), 'error');
  }
}

async function onRemoveOfficialPack() {
  try {
    if (window.__TAURI_INTERNALS__) {
      const choice = await showConfirmDialog({
        title: 'Удалить сборку?',
        message: 'Удалить все файлы сборки LemiSborka?',
        buttons: [
          { label: 'Да', value: true, variant: 'danger' },
          { label: 'Нет', value: false, variant: 'ghost' },
        ],
      });
      if (!choice) return;
    }
    await uninstallOfficialPack();
    pushNotification('Сборка удалена');
  } catch (err) {
    pushNotification(String(err), 'error');
  }
}

const importCode = ref('');
const importPreview = ref(null);
const importChecking = ref(false);
const importError = ref('');

async function onCheckCode() {
  const code = importCode.value.trim();
  if (!code) return;
  importChecking.value = true;
  importError.value = '';
  importPreview.value = null;
  try {
    importPreview.value = await previewImportCode(code);
  } catch (err) {
    importError.value = String(err);
  } finally {
    importChecking.value = false;
  }
}

watch(
  () => modsState.pendingImportCode,
  (code) => {
    if (!code) return;
    importCode.value = code;
    modsState.pendingImportCode = null;
    onCheckCode();
  },
  { immediate: true },
);

async function onApplyImport() {
  const ids = importPreview.value?.items.filter((i) => i.resolved).map((i) => i.id) ?? [];
  if (ids.length === 0 || modsState.applying) return;
  try {
    await applyImportCode(importCode.value.trim(), ids, importPreview.value?.configs);
    pushNotification('Сборка установлена');
    importPreview.value = null;
    importCode.value = '';
  } catch (err) {
    pushNotification(String(err), 'error');
  }
}

async function onRefresh() {
  refreshing.value = true;
  try {
    await Promise.all([loadModCatalog({ force: true }), loadOfficialPack()]);
  } finally {
    refreshing.value = false;
  }
}

onMounted(() => {
  loadModCatalog();
  loadOfficialPack();
  nextTick(onScroll);
});

// Catalog/pack load async, so re-check fade state once the content actually renders
watch(
  () => [modsState.catalog, modsState.officialPack],
  () => nextTick(onScroll),
);
</script>

<template>
  <div class="mods">
    <div class="head">
      <div>
        <h1>Моды</h1>
        <p class="sub">Опциональные клиентские моды — сервер работает и без них</p>
      </div>
      <button class="refresh" :disabled="refreshing || modsState.loading" @click="onRefresh">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 4a8 8 0 1 0 7.75 6h-2.08A6 6 0 1 1 12 6V2l5 4-5 4V6Z"/></svg>
        {{ refreshing ? 'Обновляю...' : 'Обновить' }}
      </button>
    </div>

    <div class="mods-scroll-wrap">
      <div class="fade fade-top" :class="{ show: topFaded }"></div>

      <div class="mods-scroll" ref="scrollEl" @scroll="onScroll">
        <Transition name="pack-fade">
        <section v-if="modsState.officialPack" class="pack-card">
          <div class="pack-info">
            <span class="pack-name">{{ modsState.officialPack.name }}</span>
            <span class="pack-version">v{{ modsState.officialPack.version }} · {{ formatFileSize(modsState.officialPack.file_size) }}</span>
            <ul v-if="modsState.officialPack.changelog.length" class="pack-changelog">
              <li v-for="(line, i) in modsState.officialPack.changelog" :key="i">{{ line }}</li>
            </ul>
          </div>
          <div class="pack-actions">
            <button
              class="pack-btn"
              :class="{ highlight: modsState.highlightPackInstall }"
              :disabled="modsState.applying || modsState.installedOfficialVersion === modsState.officialPack.version"
              @click="onApplyOfficialPack"
            >
              {{
                modsState.installedOfficialVersion === modsState.officialPack.version
                  ? 'Установлено'
                  : modsState.installedOfficialVersion
                    ? 'Обновить'
                    : 'Установить'
              }}
            </button>
            <button v-if="modsState.installedOfficialVersion" class="pack-remove-btn" :disabled="modsState.applying" @click="onRemoveOfficialPack">
              Удалить
            </button>
          </div>
        </section>
        </Transition>

        <section class="import-card">
          <div class="import-card-head">
            <span class="section-title">Импорт по коду</span>
            <button class="site-link" @click="openExternal('https://lemicraft.ru/mods')">Составить сборку на сайте</button>
          </div>
          <div class="import-row">
            <input v-model="importCode" type="text" placeholder="Код сборки" spellcheck="false" @keyup.enter="onCheckCode" />
            <button class="secondary-btn" :disabled="!importCode.trim() || importChecking" @click="onCheckCode">
              {{ importChecking ? 'Проверяю...' : 'Проверить' }}
            </button>
          </div>
          <p v-if="importError" class="error-text">{{ importError }}</p>
          <div class="import-preview-wrap" :class="{ expanded: !!importPreview }">
            <div class="import-preview-inner">
              <div v-if="importPreview" class="import-preview">
                <span v-for="item in importPreview.items" :key="item.id" class="import-chip" :class="{ unresolved: !item.resolved }">
                  {{ item.name }}
                </span>
                <span v-if="importPreview.configs" class="import-chip configs-chip">+ ресурсы/конфиги</span>
                <button class="pick-btn" :disabled="modsState.applying" @click="onApplyImport">Установить сборку</button>
              </div>
            </div>
          </div>
        </section>

        <p v-if="modsState.error" class="error-text standalone">{{ modsState.error }}</p>

        <div class="category-filter">
          <button v-for="cat in CATEGORIES" :key="cat.id" class="cat-btn" :class="{ active: activeCategory === cat.id }" @click="activeCategory = cat.id">
            {{ cat.label }}
          </button>
        </div>

        <TransitionGroup tag="div" name="mod-card" class="mod-grid">
          <article
            v-for="entry in filteredCatalog"
            :key="entry.id"
            class="mod-card"
            :class="{ selected: isInstalled(entry), conflict: !isInstalled(entry) && conflictingNames(entry).length, busy: modsState.applying }"
            :title="conflictingNames(entry).length ? `Конфликт с: ${conflictingNames(entry).join(', ')}` : ''"
            @click="onToggleCard(entry)"
          >
            <div class="mod-card-head">
              <span class="mod-icon">
                <img :src="iconUrl(entry)" :alt="entry.name" loading="lazy" @error="hideBrokenIcon" />
              </span>
              <div class="mod-card-title">
                <span class="mod-name">{{ entry.name }}</span>
                <div class="mod-badges">
                  <span class="badge mono">{{ entry.version }}</span>
                  <span v-if="entry.configs.length" class="badge configs">конфиг</span>
                  <span v-if="isInstalled(entry) && isPackOnly(entry.id)" class="badge pack-badge">из сборки</span>
                  <span v-if="!isInstalled(entry) && conflictingNames(entry).length" class="badge conflict-badge">конфликт</span>
                </div>
              </div>
              <div class="mod-card-actions">
                <button class="mod-link" title="Открыть на Modrinth" @click.stop="openExternal(modrinthUrl(entry))">
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><path d="M14 3h7v7h-2V6.4l-9.3 9.3-1.4-1.4L17.6 5H14V3ZM5 5h6v2H5v12h12v-6h2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z"/></svg>
                </button>
                <span class="mod-check" :class="{ checked: isInstalled(entry) }">
                  <svg v-if="isInstalled(entry)" viewBox="0 0 24 24" width="12" height="12" fill="currentColor"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z"/></svg>
                </span>
              </div>
            </div>
            <p class="mod-desc">{{ entry.description }}</p>
          </article>
        </TransitionGroup>
        <p v-if="filteredCatalog.length === 0 && !modsState.loading" class="empty-state">Пока пусто</p>
      </div>

      <div class="fade fade-bottom" :class="{ show: bottomFaded }"></div>
    </div>
  </div>
</template>

<style scoped>
.mods {
  display: flex;
  flex-direction: column;
  gap: 20px;
  height: 100%;
  min-height: 0;
}

.mods-scroll-wrap {
  position: relative;
  min-height: 0;
  flex: 1;
}

.mods-scroll {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.fade {
  position: absolute;
  left: 0;
  right: 10px;
  height: 14px;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease;
  z-index: 1;
}
.fade.show {
  opacity: 1;
}
.fade-top {
  top: 0;
  background: linear-gradient(var(--bg), transparent);
}
.fade-bottom {
  bottom: 0;
  background: linear-gradient(transparent, var(--bg));
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

.sub {
  color: var(--text-muted);
  font-size: 14px;
  margin-top: 6px;
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
  padding: 6px 8px;
  border-radius: 8px;
  transition: background 0.15s ease, color 0.15s ease;
}
.refresh:hover:not(:disabled) {
  background: var(--surface-2);
  color: var(--text);
}
.refresh:disabled {
  opacity: 0.5;
  cursor: default;
}

.pack-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 18px 20px;
  border-radius: var(--radius-md);
  background: var(--surface);
  border: 1px solid var(--border);
  flex-shrink: 0;
}

.pack-fade-enter-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}
.pack-fade-enter-from {
  opacity: 0;
  transform: translateY(6px);
}

.pack-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.pack-remove-btn {
  padding: 10px 16px;
  border-radius: 9px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.pack-remove-btn:hover:not(:disabled) {
  background: var(--bad-soft);
  color: var(--bad);
}
.pack-remove-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.pack-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.pack-name {
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}

.pack-version {
  font-size: 12px;
  color: var(--text-faint);
}

.pack-changelog {
  margin-top: 6px;
  padding-left: 16px;
  color: var(--text-muted);
  font-size: 12.5px;
  line-height: 1.5;
}

.pack-btn {
  flex-shrink: 0;
  padding: 10px 18px;
  border-radius: 9px;
  border: none;
  background: var(--accent);
  color: var(--accent-text-on);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.15s ease;
}
.pack-btn:hover:not(:disabled) {
  background: var(--accent-hover);
}
.pack-btn:disabled {
  opacity: 0.6;
  cursor: default;
}
.pack-btn.highlight {
  animation: pack-btn-pulse 0.6s ease 3;
}
@keyframes pack-btn-pulse {
  0%, 100% {
    box-shadow: 0 0 0 0 var(--accent);
    transform: scale(1);
  }
  50% {
    box-shadow: 0 0 0 8px transparent;
    transform: scale(1.06);
  }
}

@media (prefers-reduced-motion: reduce) {
  .pack-btn.highlight {
    animation: none;
  }
}

.section-title {
  display: block;
  font-size: 11.5px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--text-faint);
  margin-bottom: 10px;
}

.import-card {
  flex-shrink: 0;
  padding: 16px 18px;
  border-radius: var(--radius-md);
  background: var(--surface);
  border: 1px solid var(--border);
}

.import-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}
.import-card-head .section-title {
  margin-bottom: 0;
}

.site-link {
  background: none;
  border: none;
  color: var(--text-faint);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  padding: 2px 0;
  transition: color 0.15s ease;
}
.site-link:hover {
  color: var(--accent);
}

.import-row {
  display: flex;
  gap: 8px;
}

.import-row input {
  flex: 1;
  padding: 9px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
}
.import-row input:focus {
  border-color: var(--accent);
}

.secondary-btn {
  flex-shrink: 0;
  padding: 9px 16px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.secondary-btn:hover:not(:disabled) {
  background: var(--surface-hover);
}
.secondary-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.import-preview-wrap {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 0.25s ease;
}
.import-preview-wrap.expanded {
  grid-template-rows: 1fr;
}
.import-preview-inner {
  overflow: hidden;
  min-height: 0;
}

.import-preview {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-top: 12px;
}

.import-chip {
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--surface-2);
  border: 1px solid var(--border);
  font-size: 12px;
  color: var(--text);
}
.import-chip.unresolved {
  color: var(--text-faint);
  border-style: dashed;
}
.import-chip.configs-chip {
  background: var(--good-soft);
  border-color: transparent;
  color: var(--good);
  font-weight: 600;
}

.import-preview .pick-btn {
  margin-left: auto;
}

.pick-btn {
  flex-shrink: 0;
  padding: 9px 16px;
  border-radius: 8px;
  border: none;
  background: var(--accent);
  color: var(--accent-text-on);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}
.pick-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.error-text {
  color: var(--bad);
  font-size: 13px;
  margin-top: 8px;
}
.error-text.standalone {
  padding: 20px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  text-align: center;
}

.category-filter {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  flex-shrink: 0;
}

.cat-btn {
  padding: 7px 14px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
}
.cat-btn:hover {
  background: var(--surface-hover);
  color: var(--text);
}
.cat-btn.active {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--accent-text-on);
}

.empty-state {
  padding: 20px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  color: var(--text-faint);
  font-size: 13.5px;
  text-align: center;
  flex-shrink: 0;
}

.mod-grid {
  position: relative;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 10px;
  flex-shrink: 0;
}

.mod-card-enter-active,
.mod-card-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.mod-card-move {
  transition: transform 0.2s ease;
}
.mod-card-enter-from,
.mod-card-leave-to {
  opacity: 0;
}

.mod-card {
  padding: 12px;
  border-radius: var(--radius-md);
  background: var(--surface);
  border: 1px solid var(--border);
  cursor: pointer;
  transition: border-color 0.15s ease, background 0.15s ease, opacity 0.15s ease;
}
.mod-card:hover {
  border-color: var(--text-faint);
}
.mod-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.mod-card.conflict {
  opacity: 0.55;
  cursor: not-allowed;
}
.mod-card.busy {
  pointer-events: none;
  opacity: 0.7;
}

.mod-card-head {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  margin-bottom: 8px;
}

.mod-icon {
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  border-radius: 9px;
  overflow: hidden;
  background: var(--surface-2);
  display: flex;
  align-items: center;
  justify-content: center;
}
.mod-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.mod-card-title {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.mod-name {
  font-size: 13.5px;
  font-weight: 700;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mod-badges {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.badge {
  font-size: 10.5px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--surface-2);
  color: var(--text-faint);
}
.badge.mono {
  font-family: var(--font-mono);
  background: rgba(99, 140, 255, 0.15);
  color: #8fa8ff;
}
.badge.configs {
  background: var(--good-soft);
  color: var(--good);
}
.badge.conflict-badge {
  background: var(--bad-soft);
  color: var(--bad);
}
.badge.pack-badge {
  background: rgba(231, 200, 115, 0.15);
  color: var(--gold);
}

.mod-card-actions {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 6px;
}

.mod-link {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  border: none;
  background: none;
  color: var(--text-faint);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.mod-link:hover {
  background: var(--surface-2);
  color: var(--text);
}

.mod-check {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1.5px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent-text-on);
  transition: background 0.15s ease, border-color 0.15s ease;
}
.mod-check.checked {
  background: var(--accent);
  border-color: var(--accent);
}

.mod-desc {
  font-size: 11.5px;
  color: var(--text-muted);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

@media (prefers-reduced-motion: reduce) {
  .mod-card-enter-active,
  .mod-card-leave-active,
  .mod-card-move {
    transition: none;
  }
}
</style>
