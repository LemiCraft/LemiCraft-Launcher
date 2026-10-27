<script setup>
import { computed, onMounted, onUnmounted, watch } from 'vue';
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

const verdict = computed(() => {
  const probe = entryPoints.probe;
  if (!probe) return null;
  const best = probe.points.find((p) => p.id === probe.best);
  if (!best) return { text: 'Ни один путь не ответил', level: 'bad' };
  const ms = best.ping_ms == null ? '' : `, ${Math.round(best.ping_ms)} мс`;
  if (probe.suggestion) return { text: `Рекомендуем: ${best.label}${ms}`, level: 'good' };
  if (best.id === 'direct') return { text: `Лучший путь сейчас: напрямую${ms}`, level: 'good' };
  return { text: `Прямое подключение подходит, разница небольшая. Быстрее всех: ${best.label}${ms}`, level: 'good' };
});

const footerText = computed(() => (entryPoints.checking && entryPoints.probe ? 'Проверяю...' : checkedText(entryPoints.probe)));
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="open" class="overlay" @click.self="open = false">
        <div class="entry-modal">
          <div class="progress" :class="{ on: entryPoints.checking }"></div>
          <button class="close-btn" aria-label="Закрыть" @click="open = false">
            <svg viewBox="0 0 10 10" width="11" height="11"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3" /></svg>
          </button>
          <h2>Подбор подключения</h2>
          <p class="lead">Как быстро и стабильно отвечает сервер напрямую и через другие страны. Это только подсказка: игра по-прежнему подключается как раньше</p>

          <div class="verdict-slot">
            <Transition name="swap" mode="out-in">
              <div v-if="verdict" :key="verdict.text" class="verdict" :class="verdict.level">{{ verdict.text }}</div>
            </Transition>
          </div>

          <div class="points">
            <template v-if="entryPoints.probe">
              <div
                v-for="(point, i) in entryPoints.probe.points"
                :key="point.id"
                class="point"
                :class="{ best: point.id === entryPoints.probe.best, checking: entryPoints.checking }"
                :style="{ '--i': i }"
              >
                <div class="point-text">
                  <span class="point-title">
                    {{ point.label }}
                    <Transition name="tag">
                      <span v-if="point.id === entryPoints.probe.best" class="tag">лучший</span>
                    </Transition>
                  </span>
                  <span class="point-host">{{ point.host }}</span>
                </div>
                <Transition name="swap" mode="out-in">
                  <span :key="pingText(point)" class="point-ping">{{ pingText(point) }}</span>
                </Transition>
                <Transition name="swap" mode="out-in">
                  <span :key="entryState(point).text" class="point-state" :class="entryState(point).level">{{ entryState(point).text }}</span>
                </Transition>
              </div>
            </template>
            <template v-else>
              <div v-for="n in 4" :key="n" class="point" :style="{ '--i': n - 1 }">
                <div class="point-text">
                  <span class="bone wide"></span>
                  <span class="bone narrow"></span>
                </div>
                <span class="bone ping"></span>
                <span class="bone state"></span>
              </div>
            </template>
          </div>

          <div class="footer">
            <Transition name="swap" mode="out-in">
              <span :key="footerText" class="checked">{{ footerText }}</span>
            </Transition>
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
  overflow: hidden;
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

/* a thin line that runs along the top edge while the check is going */
.progress {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  overflow: hidden;
  opacity: 0;
  transition: opacity 0.3s ease;
}
.progress.on {
  opacity: 1;
}
.progress.on::after {
  content: '';
  position: absolute;
  inset: 0;
  width: 35%;
  background: var(--accent);
  border-radius: 2px;
  animation: progress-run 1.2s ease-in-out infinite;
}
@keyframes progress-run {
  from { transform: translateX(-100%); }
  to { transform: translateX(290%); }
}

.verdict-slot {
  min-height: 38px;
  display: flex;
}
.verdict {
  flex: 1;
  display: flex;
  align-items: center;
  padding: 9px 12px;
  border-radius: 9px;
  font-size: 13px;
  font-weight: 600;
  line-height: 1.35;
}
.verdict.accent {
  background: var(--accent-soft);
  color: var(--accent);
}
.verdict.good {
  background: var(--good-soft, rgba(80, 200, 120, 0.12));
  color: var(--good);
}
.verdict.bad {
  background: var(--bad-soft);
  color: var(--bad);
}

.points {
  display: flex;
  flex-direction: column;
  margin-top: 2px;
}

.point {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 11px 10px;
  margin: 0 -10px;
  border-radius: 10px;
  animation: row-in 0.38s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  animation-delay: calc(var(--i) * 55ms);
  transition: background 0.35s ease;
}
.point.best {
  background: var(--surface-2);
}
@keyframes row-in {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

/* values breathe while a new check is running, instead of changing in front of the eyes */
.point.checking .point-ping,
.point.checking .point-state {
  animation: breathe 1.2s ease-in-out infinite;
}
@keyframes breathe {
  0%, 100% { opacity: 0.45; }
  50% { opacity: 1; }
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
  display: inline-block;
  margin-left: 8px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11px;
  font-weight: 600;
}
.tag-enter-active,
.tag-leave-active {
  transition: opacity 0.25s ease, transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.tag-enter-from,
.tag-leave-to {
  opacity: 0;
  transform: scale(0.85);
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

.bone {
  display: block;
  height: 12px;
  border-radius: 6px;
  background: var(--surface-2);
  animation: breathe 1.4s ease-in-out infinite;
  animation-delay: calc(var(--i, 0) * 120ms);
}
.bone.wide { width: 110px; height: 14px; }
.bone.narrow { width: 140px; height: 10px; margin-top: 4px; }
.bone.ping { width: 50px; }
.bone.state { width: 84px; }

.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 6px;
  min-height: 36px;
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
  transition: background 0.15s ease, opacity 0.2s ease;
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

/* texts and numbers that change are cross-faded */
.swap-enter-active,
.swap-leave-active {
  transition: opacity 0.18s ease;
}
.swap-enter-from,
.swap-leave-to {
  opacity: 0;
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-active .entry-modal,
.modal-leave-active .entry-modal {
  transition: transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
.modal-enter-from .entry-modal,
.modal-leave-to .entry-modal {
  opacity: 0;
  transform: translateY(12px) scale(0.97);
}

@media (prefers-reduced-motion: reduce) {
  .point,
  .point.checking .point-ping,
  .point.checking .point-state,
  .bone,
  .progress.on::after {
    animation: none;
  }
  .modal-enter-active .entry-modal,
  .modal-leave-active .entry-modal,
  .swap-enter-active,
  .swap-leave-active,
  .tag-enter-active,
  .tag-leave-active {
    transition: none;
  }
}
</style>
