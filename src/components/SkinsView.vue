<script setup>
import { computed, nextTick, onMounted, reactive, ref, watch } from 'vue';
import { accountStore } from '../store/account.js';
import {
  skinsState,
  loadSkins,
  applySkin,
  deleteSkin,
  uploadSkin,
  openElybyWebLogin,
  finishElybyWebLogin,
  forgetElybySession,
  checkElybySession,
} from '../store/skins.js';
import { renderSkinThumbnail, fetchImageDataUri } from '../utils/skinThumbnail.js';
import { readImageFile, detectSkinModel } from '../utils/skinUpload.js';
import { pushNotification } from '../store/notifications.js';
import { showConfirmDialog } from '../store/confirmDialog.js';

const MAX_SKIN_FILE_BYTES = 2 * 1024 * 1024;

const isElyBy = computed(() => accountStore.provider === 'Ely.by');
const needsElybyLogin = computed(() => isElyBy.value && !skinsState.elybySessionReady);
const feedState = computed(() => (skinsState.loading ? 'loading' : skinsState.error ? 'error' : 'content'));
const SKELETON_COUNT = 6;

const fileInput = ref(null);
const uploadModalOpen = ref(false);
const uploadModel = ref('steve');
const uploadName = ref('');
const uploadFile = ref(null);
const uploadPreviewUrl = ref('');
const uploading = ref(false);
const uploadError = ref('');
const connectingElyby = ref(false);
const busySkinId = ref(null);
const dragOver = ref(false);
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

// skin.id -> rendered front-view data URL; component-local, so it resets on every mount
const renderedThumbs = reactive({});

function loadThumbs(items, force = false) {
  for (const skin of items) {
    if (!force && renderedThumbs[skin.id]) continue;
    // thumbnailUrl differs from fileUrl when the backend already rendered a proper front view
    const hasRealThumb = skin.thumbnailUrl && skin.thumbnailUrl !== skin.fileUrl;
    const load = hasRealThumb
      ? fetchImageDataUri(skin.thumbnailUrl, force)
      : renderSkinThumbnail(skin.fileUrl, skin.model === 'alex', force);
    load.then((dataUrl) => {
      if (dataUrl) renderedThumbs[skin.id] = dataUrl;
    });
  }
}

onMounted(() => {
  if (accountStore.loggedIn) loadSkins();
  loadThumbs(skinsState.items);
  nextTick(onScroll);
});

watch(
  () => [accountStore.loggedIn, accountStore.provider],
  () => {
    if (accountStore.loggedIn) loadSkins();
  },
);

watch(uploadModalOpen, (open) => {
  if (!open) resetPreview();
});

watch(
  () => skinsState.items,
  (items) => {
    loadThumbs(items);
    nextTick(onScroll);
  },
  { deep: true },
);

function resetPreview() {
  if (uploadPreviewUrl.value) URL.revokeObjectURL(uploadPreviewUrl.value);
  uploadPreviewUrl.value = '';
}

function openUploadModal() {
  resetPreview();
  uploadFile.value = null;
  uploadName.value = '';
  uploadModel.value = 'steve';
  uploadError.value = '';
  uploadModalOpen.value = true;
}

function pickFile() {
  fileInput.value?.click();
}

async function useFile(file) {
  if (!file) return;
  uploadError.value = '';

  if (file.size > MAX_SKIN_FILE_BYTES) {
    uploadError.value = `Размер файла превышает ${(MAX_SKIN_FILE_BYTES / 1024 / 1024).toFixed(0)} МБ (сейчас ${(file.size / 1024 / 1024).toFixed(2)} МБ)`;
    return;
  }

  let loaded;
  try {
    loaded = await readImageFile(file);
  } catch {
    uploadError.value = 'Не удалось загрузить изображение';
    return;
  }

  const { img, url, width, height } = loaded;
  const validSize = (width === 64 && height === 64) || (width === 64 && height === 32);
  if (!validSize) {
    URL.revokeObjectURL(url);
    uploadError.value = `Неверные размеры изображения: ${width}x${height} — требуется 64x64 или 64x32`;
    return;
  }

  resetPreview();
  uploadPreviewUrl.value = url;
  uploadFile.value = file;
  uploadModel.value = detectSkinModel(img, width, height);
  if (!uploadName.value) uploadName.value = file.name.replace(/\.[^.]+$/, '');
}

function onFileChosen(event) {
  const file = event.target.files?.[0];
  event.target.value = '';
  useFile(file);
}

function onDrop(event) {
  dragOver.value = false;
  useFile(event.dataTransfer?.files?.[0]);
}

async function submitUpload() {
  if (!uploadFile.value || uploading.value) return;
  uploading.value = true;
  uploadError.value = '';
  try {
    await uploadSkin(uploadFile.value, uploadName.value.trim() || uploadFile.value.name, uploadModel.value);
    uploadModalOpen.value = false;
  } catch (err) {
    uploadError.value = String(err);
    if (isElyBy.value) await checkElybySession();
  } finally {
    uploading.value = false;
  }
}

async function onApply(skin) {
  busySkinId.value = skin.id;
  skinsState.error = '';
  try {
    await applySkin(skin);
    pushNotification(`Скин «${skin.name}» применён`);
  } catch (err) {
    skinsState.error = String(err);
    if (isElyBy.value) await checkElybySession();
  } finally {
    busySkinId.value = null;
  }
}

async function onDelete(skin) {
  if (window.__TAURI_INTERNALS__) {
    const choice = await showConfirmDialog({
      title: 'Удалить скин?',
      message: `Удалить скин «${skin.name}»? Это действие нельзя отменить`,
      buttons: [
        { label: 'Да', value: true, variant: 'danger' },
        { label: 'Нет', value: false, variant: 'ghost' },
      ],
    });
    if (!choice) return;
  }

  busySkinId.value = skin.id;
  skinsState.error = '';
  try {
    await deleteSkin(skin);
    pushNotification(`Скин «${skin.name}» удалён`);
  } catch (err) {
    skinsState.error = String(err);
    if (isElyBy.value) await checkElybySession();
  } finally {
    busySkinId.value = null;
  }
}

async function onRefresh() {
  refreshing.value = true;
  try {
    await loadSkins({ force: true });
    loadThumbs(skinsState.items, true);
  } finally {
    refreshing.value = false;
  }
}

function formatAddedAt(iso) {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const pad = (n) => String(n).padStart(2, '0');
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

async function onConnectElyby() {
  await openElybyWebLogin();
}

async function onConfirmElybyLogin() {
  connectingElyby.value = true;
  skinsState.error = '';
  try {
    await finishElybyWebLogin();
  } catch (err) {
    skinsState.error = String(err);
  } finally {
    connectingElyby.value = false;
  }
}
</script>

<template>
  <div class="skins">
    <div class="head">
      <div>
        <h1>Скины</h1>
        <p class="sub">{{ accountStore.loggedIn ? `Ваши скины · ${accountStore.provider}` : 'Войдите в аккаунт, чтобы управлять скинами' }}</p>
      </div>
      <div v-if="accountStore.loggedIn && !needsElybyLogin" class="head-actions">
        <button class="refresh" :disabled="refreshing || skinsState.loading" @click="onRefresh">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 4a8 8 0 1 0 7.75 6h-2.08A6 6 0 1 1 12 6V2l5 4-5 4V6Z"/></svg>
          {{ refreshing ? 'Обновляю...' : 'Обновить' }}
        </button>
        <button class="upload-btn" @click="openUploadModal">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><path d="M12 3l5 5h-3v6h-4V8H7l5-5Zm-7 14h14v2H5v-2Z"/></svg>
          Загрузить скин
        </button>
      </div>
    </div>

    <div class="skins-scroll-wrap">
    <div class="fade fade-top" :class="{ show: topFaded }"></div>

    <div class="skins-scroll" ref="scrollEl" @scroll="onScroll">
    <div v-if="!accountStore.loggedIn" class="empty-state">
      <p>Войдите в аккаунт, чтобы увидеть свои скины</p>
    </div>

    <div v-else-if="needsElybyLogin" class="elyby-gate">
      <p class="gate-title">У ely.by нет собственного API для скинов</p>
      <p class="gate-text">Управление скинами возможно только через сессию сайта ely.by — войдите один раз во встроенном окне, оно закроется само, как только вы войдёте</p>
      <div class="gate-actions">
        <button class="pick-btn" :disabled="skinsState.elybyLoginPending" @click="onConnectElyby">
          {{ skinsState.elybyLoginPending ? 'Ожидание входа...' : 'Управлять на ely.by' }}
        </button>
      </div>
      <button v-if="skinsState.elybyLoginPending" class="forget-link" :disabled="connectingElyby" @click="onConfirmElybyLogin">
        {{ connectingElyby ? 'Проверяю...' : 'Не сработало само? Проверить вручную' }}
      </button>
      <p v-if="skinsState.error" class="error-text">{{ skinsState.error }}</p>
    </div>

    <template v-else>
      <div class="grid-wrap">
        <Transition name="crossfade">
          <div v-if="feedState === 'loading'" key="loading" class="grid">
            <div v-for="i in SKELETON_COUNT" :key="i" class="skin-skeleton" :style="{ animationDelay: `${i * 45}ms` }">
              <span class="sk-thumb"></span>
              <span class="sk-name"></span>
              <span class="sk-actions"></span>
            </div>
          </div>

          <p v-else-if="feedState === 'error'" key="error" class="error-text standalone">{{ skinsState.error }}</p>

          <div v-else-if="skinsState.items.length === 0" key="empty" class="empty-state">
            <p>Скинов пока нет — загрузите первый</p>
          </div>

          <TransitionGroup v-else key="content" name="card" tag="div" class="grid">
            <article v-for="skin in skinsState.items" :key="skin.id" class="skin-card" :class="{ active: skin.isActive }">
              <div class="thumb">
                <img
                  v-if="renderedThumbs[skin.id]"
                  :src="renderedThumbs[skin.id]"
                  :alt="skin.name"
                  class="thumb-rendered"
                  draggable="false"
                  @contextmenu.prevent
                />
                <span v-else class="thumb-skeleton"></span>
                <span v-if="skin.isActive" class="active-badge">Надет</span>
              </div>
              <div class="skin-info">
                <span class="skin-name">{{ skin.name }}</span>
                <span class="skin-model">
                  {{ skin.model === 'alex' ? 'Alex' : 'Steve' }}
                  <template v-if="skin.addedAt"> · {{ formatAddedAt(skin.addedAt) }}</template>
                </span>
              </div>
              <div class="skin-actions">
                <button class="apply-btn" :disabled="skin.isActive || busySkinId === skin.id" @click="onApply(skin)">
                  {{ busySkinId === skin.id ? '...' : 'Надеть' }}
                </button>
                <button class="delete-btn" :disabled="busySkinId === skin.id" @click="onDelete(skin)">
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M9 3h6l1 2h4v2H4V5h4l1-2Zm-3 6h12l-1 12H7L6 9Z"/></svg>
                </button>
              </div>
            </article>
          </TransitionGroup>
        </Transition>
      </div>

      <button v-if="isElyBy && skinsState.elybySessionReady" class="forget-link" @click="forgetElybySession">Отвязать сессию ely.by</button>
    </template>
    </div>

    <div class="fade fade-bottom" :class="{ show: bottomFaded }"></div>
    </div>

    <Transition name="modal">
      <div v-if="uploadModalOpen" class="overlay" @click.self="uploadModalOpen = false">
        <div class="upload-modal">
          <h2>Загрузить скин</h2>

          <div
            class="upload-drop"
            :class="{ filled: !!uploadFile, 'drag-over': dragOver }"
            @click="pickFile"
            @dragover.prevent="dragOver = true"
            @dragleave.prevent="dragOver = false"
            @drop.prevent="onDrop"
          >
            <img v-if="uploadPreviewUrl" :src="uploadPreviewUrl" alt="" class="upload-preview" draggable="false" />
            <span v-if="uploadFile">{{ uploadFile.name }}</span>
            <span v-else>{{ dragOver ? 'Отпустите, чтобы загрузить' : 'Перетащите PNG сюда или нажмите, чтобы выбрать' }}</span>
          </div>
          <input ref="fileInput" type="file" accept="image/png" class="hidden-input" @change="onFileChosen" />

          <label class="field-label">Название</label>
          <input v-model="uploadName" type="text" class="name-input" placeholder="Название скина" spellcheck="false" />

          <label class="field-label">Модель</label>
          <div class="model-toggle">
            <span :class="{ active: uploadModel === 'steve' }" @click="uploadModel = 'steve'">Steve</span>
            <span :class="{ active: uploadModel === 'alex' }" @click="uploadModel = 'alex'">Alex</span>
          </div>

          <p v-if="uploadError" class="error-text">{{ uploadError }}</p>

          <div class="modal-actions">
            <button class="cancel-btn" @click="uploadModalOpen = false">Отмена</button>
            <button class="pick-btn" :disabled="!uploadFile || uploading" @click="submitUpload">{{ uploading ? 'Загружаю...' : 'Загрузить' }}</button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.skins {
  display: flex;
  flex-direction: column;
  gap: 20px;
  height: 100%;
  min-height: 0;
}

.skins-scroll-wrap {
  position: relative;
  min-height: 0;
  flex: 1;
}

.skins-scroll {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
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

.head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
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

.upload-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: 13.5px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
  flex-shrink: 0;
}
.upload-btn:hover:not(:disabled) {
  background: var(--surface-hover);
}
.upload-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

@keyframes card-in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

.empty-state {
  padding: 20px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  color: var(--text-faint);
  font-size: 13.5px;
  text-align: center;
}

.error-text {
  color: var(--bad);
  font-size: 13px;
}
.error-text.standalone {
  padding: 20px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  text-align: center;
}

.active-badge {
  position: absolute;
  top: 8px;
  right: 8px;
  background: var(--accent);
  color: var(--accent-text-on);
  font-size: 10.5px;
  font-weight: 700;
  padding: 3px 8px;
  border-radius: 999px;
}

.elyby-gate {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 28px 24px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  text-align: center;
}

.gate-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}

.gate-text {
  max-width: 420px;
  font-size: 13px;
  color: var(--text-muted);
  line-height: 1.5;
}

.gate-actions {
  display: flex;
  gap: 10px;
  margin-top: 6px;
}

.grid-wrap {
  position: relative;
}

.crossfade-enter-active,
.crossfade-leave-active {
  transition: opacity 0.25s ease;
}
.crossfade-enter-from,
.crossfade-leave-to {
  opacity: 0;
}
.crossfade-leave-active {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
}

.card-enter-active,
.card-leave-active {
  transition: opacity 0.25s ease, transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}
.card-move {
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}
.card-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.card-leave-to {
  opacity: 0;
  transform: scale(0.96);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 14px;
}

.skin-card,
.skin-skeleton {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  animation: card-in 0.35s cubic-bezier(0.16, 1, 0.3, 1) backwards;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.skin-card.active {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

.sk-thumb,
.sk-name,
.sk-actions {
  display: block;
  border-radius: 8px;
  background: var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}
.sk-thumb {
  aspect-ratio: 1;
}
.sk-name {
  height: 13px;
  width: 70%;
}
.sk-actions {
  height: 26px;
}

.thumb {
  position: relative;
  aspect-ratio: 1;
  border-radius: 8px;
  overflow: hidden;
  background: var(--surface-2);
}
.thumb-rendered {
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.thumb-skeleton {
  display: block;
  width: 100%;
  height: 100%;
  background: var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}

.skin-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.skin-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.skin-model {
  font-size: 11.5px;
  color: var(--text-faint);
}

.skin-actions {
  display: flex;
  gap: 6px;
}

.apply-btn {
  flex: 1;
  padding: 7px 0;
  border-radius: 7px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.apply-btn:hover:not(:disabled) {
  background: var(--surface-hover);
}
.apply-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.delete-btn {
  flex-shrink: 0;
  width: 30px;
  border-radius: 7px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--bad);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s ease;
}
.delete-btn:hover:not(:disabled) {
  background: var(--bad-soft);
}
.delete-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.forget-link {
  align-self: flex-start;
  background: none;
  border: none;
  color: var(--text-faint);
  font-size: 12px;
  cursor: pointer;
  padding: 4px 0;
  flex-shrink: 0;
}
.forget-link:hover {
  color: var(--text-muted);
}

.overlay {
  position: fixed;
  inset: 0;
  background: rgba(8, 7, 9, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.upload-modal {
  width: min(360px, 88vw);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 22px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
}

.modal-enter-active .upload-modal,
.modal-leave-active .upload-modal {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.modal-enter-from .upload-modal,
.modal-leave-to .upload-modal {
  opacity: 0;
  transform: scale(0.96) translateY(6px);
}
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.15s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.upload-modal h2 {
  font-size: 17px;
  font-weight: 700;
  color: var(--text);
  margin-bottom: 6px;
}

.upload-drop {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 22px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  text-align: center;
  font-size: 13px;
  color: var(--text-faint);
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease;
}

.upload-preview {
  width: 64px;
  height: 64px;
  image-rendering: pixelated;
  border-radius: 6px;
  background: var(--surface-2);
}
.upload-drop:hover {
  border-color: var(--accent);
}
.upload-drop.filled {
  color: var(--text);
  border-style: solid;
}
.upload-drop.drag-over {
  border-color: var(--accent);
  border-style: solid;
  background: var(--accent-soft);
  color: var(--text);
}

.hidden-input {
  display: none;
}

.field-label {
  font-size: 11.5px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--text-faint);
  margin-top: 6px;
}

.name-input {
  padding: 9px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
}
.name-input:focus {
  border-color: var(--accent);
}

.model-toggle {
  display: flex;
  padding: 3px;
  border-radius: 8px;
  background: var(--surface-2);
  align-self: flex-start;
}
.model-toggle span {
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.model-toggle span.active {
  background: var(--accent);
  color: var(--accent-text-on);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 10px;
}

.cancel-btn {
  padding: 9px 16px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.cancel-btn:hover {
  background: var(--surface-2);
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

@media (prefers-reduced-motion: reduce) {
  .skin-card,
  .skin-skeleton,
  .featured {
    animation: none;
  }
  .sk-thumb,
  .sk-name,
  .sk-actions,
  .thumb-skeleton {
    animation: none;
  }
  .crossfade-enter-active,
  .crossfade-leave-active,
  .card-enter-active,
  .card-leave-active,
  .card-move {
    transition: none;
  }
}
</style>
