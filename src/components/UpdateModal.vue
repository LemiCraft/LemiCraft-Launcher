<script setup>
import { computed } from 'vue';
import { updateState, startUpdateDownload, dismissUpdate } from '../store/update.js';
import { formatFileSize } from '../utils/format.js';

const releaseDate = computed(() => {
  const raw = updateState.info?.release_date;
  if (!raw) return '';
  const d = new Date(raw);
  if (Number.isNaN(d.getTime())) return '';
  return d.toLocaleDateString('ru-RU', { day: '2-digit', month: 'long', year: 'numeric' });
});

const progressText = computed(() => {
  if (updateState.installing) return 'Установка... окно закроется автоматически';
  if (!updateState.downloading) return '';
  if (updateState.percent == null || updateState.percent < 0) {
    return `Скачивание... ${formatFileSize(updateState.bytes)}`;
  }
  return updateState.percent >= 100 ? 'Запуск установщика...' : `Скачивание... ${Math.round(updateState.percent)}%`;
});

const indeterminate = computed(() => updateState.installing || updateState.percent == null || updateState.percent < 0);
</script>

<template>
  <Transition name="modal">
    <div v-if="updateState.available" class="overlay" @click.self="dismissUpdate">
      <div class="update-modal">
        <span v-if="updateState.info?.is_required" class="required-badge">Обязательное обновление</span>

        <h2>Доступно обновление</h2>
        <p class="version-line mono">{{ updateState.info?.version ? `→ ${updateState.info.version}` : '' }}</p>

        <div class="meta-row">
          <span>{{ formatFileSize(updateState.info?.file_size) }}</span>
          <span v-if="releaseDate">{{ releaseDate }}</span>
        </div>

        <ul v-if="updateState.info?.changelog?.length" class="changelog">
          <li v-for="(line, i) in updateState.info.changelog" :key="i">{{ line }}</li>
        </ul>

        <div v-if="updateState.downloading || updateState.installing" class="progress-wrap">
          <div class="progress-bar" :class="{ indeterminate }">
            <div v-if="!indeterminate" class="progress-fill" :style="{ width: `${updateState.percent}%` }"></div>
          </div>
          <span class="progress-text">{{ progressText }}</span>
        </div>

        <p v-if="updateState.error" class="error-text">{{ updateState.error }}</p>

        <div class="modal-actions">
          <button v-if="!updateState.info?.is_required" class="cancel-btn" :disabled="updateState.downloading" @click="dismissUpdate">Позже</button>
          <button class="update-btn" :disabled="updateState.downloading || updateState.installing" @click="startUpdateDownload">
            {{ updateState.downloading ? 'Скачиваю...' : 'Обновить сейчас' }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(8, 7, 9, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.update-modal {
  position: relative;
  width: min(420px, 88vw);
  max-height: 82vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 22px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
}

.required-badge {
  align-self: flex-start;
  padding: 3px 9px;
  border-radius: 999px;
  background: var(--bad-soft);
  color: var(--bad);
  font-size: 11px;
  font-weight: 700;
}

.update-modal h2 {
  font-size: 18px;
  font-weight: 800;
  color: var(--text);
}

.version-line {
  color: var(--accent);
  font-size: 14px;
  font-weight: 600;
}

.meta-row {
  display: flex;
  gap: 12px;
  font-size: 12.5px;
  color: var(--text-muted);
}

.changelog {
  margin: 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 13px;
  color: var(--text-muted);
  line-height: 1.5;
}

.progress-wrap {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.progress-bar {
  height: 6px;
  border-radius: 999px;
  background: var(--surface-2);
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.2s ease;
}
.progress-bar.indeterminate {
  position: relative;
}
.progress-bar.indeterminate::after {
  content: '';
  position: absolute;
  inset: 0;
  width: 40%;
  background: var(--accent);
  border-radius: 999px;
  animation: indeterminate-slide 1.1s ease-in-out infinite;
}
@keyframes indeterminate-slide {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(350%); }
}

.progress-text {
  font-size: 12px;
  color: var(--text-faint);
}

.error-text {
  color: var(--bad);
  font-size: 13px;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 4px;
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
.cancel-btn:hover:not(:disabled) {
  background: var(--surface-2);
}
.cancel-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.update-btn {
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
.update-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.15s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .progress-bar.indeterminate::after {
    animation: none;
  }
}
</style>
