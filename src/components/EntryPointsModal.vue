<script setup>
import { onMounted, onUnmounted, watch } from 'vue';
import { entryPoints, entryState, initEntryPoints, runEntryProbe } from '../store/entryPoints.js';

const open = defineModel({ type: Boolean, default: false });

function onKeydown(event) {
  if (event.key === 'Escape' && open.value) open.value = false;
}
onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));

// the check runs when the window is opened, unless the last result is fresh
watch(open, (value) => {
  if (value) initEntryPoints();
});

function pingText(point) {
  return point.ping_ms == null ? '—' : `${Math.round(point.ping_ms)} мс`;
}

function checkedText(probe) {
  if (!probe) return '';
  const minutes = Math.floor((Date.now() / 1000 - probe.checked_at) / 60);
  return minutes < 1 ? 'Проверено только что' : `Проверено ${minutes} мин назад`;
}
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="open" class="overlay" @click.self="open = false">
        <div class="entry-modal">
          <button class="close-btn" aria-label="Закрыть" @click="open = false">
            <svg viewBox="0 0 10 10" width="11" height="11"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3" /></svg>
          </button>
          <h2>Пути к серверу</h2>
          <p class="lead">Задержка и стабильность каждого пути. Подключение по умолчанию не меняется, это только подсказка</p>

          <div class="points">
            <div v-for="point in entryPoints.probe?.points ?? []" :key="point.id" class="point">
              <div class="point-text">
                <span class="point-title">
                  {{ point.label }}
                  <span v-if="point.id === entryPoints.probe.suggestion" class="tag">лучший сейчас</span>
                </span>
                <span class="point-host">{{ point.host }}</span>
              </div>
              <span class="point-ping">{{ pingText(point) }}</span>
              <span class="point-state" :class="entryState(point).level">{{ entryState(point).text }}</span>
            </div>
            <div v-if="!entryPoints.probe" class="empty">{{ entryPoints.checking ? 'Проверяю...' : 'Пока нет данных' }}</div>
          </div>

          <div class="footer">
            <span class="checked">{{ entryPoints.checking && entryPoints.probe ? 'Проверяю...' : checkedText(entryPoints.probe) }}</span>
            <button class="check-btn" :disabled="entryPoints.checking" @click="runEntryProbe">Проверить ещё раз</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
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

.entry-modal {
  position: relative;
  width: min(520px, 90vw);
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 22px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
}

h2 {
  font-size: 17px;
  font-weight: 800;
  color: var(--text);
  padding-right: 24px;
}

.lead {
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.5;
}

.points {
  display: flex;
  flex-direction: column;
  margin-top: 4px;
  min-height: 80px;
}

.point {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 11px 0;
  border-top: 1px solid var(--border);
}
.point:first-child {
  border-top: none;
}

.point-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.point-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}
.point-host {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-muted);
}

.tag {
  margin-left: 8px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11px;
  font-weight: 600;
}

.point-ping {
  min-width: 56px;
  text-align: right;
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--text-muted);
}
.point-state {
  min-width: 92px;
  text-align: right;
  font-size: 12.5px;
  font-weight: 600;
}
.point-state.good { color: var(--good); }
.point-state.warn { color: var(--gold); }
.point-state.bad { color: var(--bad); }

.empty {
  padding: 26px 0;
  text-align: center;
  color: var(--text-faint);
  font-size: 13px;
}

.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 6px;
}
.checked {
  font-size: 12.5px;
  color: var(--text-faint);
}

.check-btn {
  padding: 9px 16px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.check-btn:hover:not(:disabled) {
  background: var(--surface-2);
}
.check-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.close-btn {
  position: absolute;
  top: 14px;
  right: 14px;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  border: none;
  background: transparent;
  color: var(--text-faint);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.close-btn:hover {
  background: var(--surface-2);
  color: var(--text);
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.15s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
</style>
