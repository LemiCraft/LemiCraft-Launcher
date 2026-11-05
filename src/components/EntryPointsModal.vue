<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { entryPoints, entryState, initEntryPoints, runEntryProbe } from '../store/entryPoints.js';
import { copyText } from '../utils/clipboard.js';
import { pushNotification } from '../store/notifications.js';

const open = defineModel({ type: Boolean, default: false });

const shaking = ref(false);
let shakeTimer = null;

// A running check is not closed by a stray click outside or by Escape: the window only shakes. The cross
// still closes it; the check goes on in the background and the result is there when the window is opened again.
function close(deliberate = false) {
  if (entryPoints.checking && !deliberate) {
    shaking.value = true;
    clearTimeout(shakeTimer);
    shakeTimer = setTimeout(() => (shaking.value = false), 450);
    return;
  }
  open.value = false;
}

function onKeydown(event) {
  if (event.key === 'Escape' && open.value) close();
}
onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));

const now = ref(Date.now());
let clock = null;

// the check runs when the window is opened, unless the last result is fresh
watch(open, (value) => {
  clearInterval(clock);
  if (value) {
    now.value = Date.now();
    clock = setInterval(() => (now.value = Date.now()), 30000);
    initEntryPoints();
  }
});
onUnmounted(() => {
  clearInterval(clock);
  clearTimeout(shakeTimer);
});

async function copyHost(host) {
  if (await copyText(host)) pushNotification('Адрес скопирован');
  else pushNotification('Не удалось скопировать адрес', 'error');
}

function pingText(point) {
  return point.ping_ms == null ? '—' : `${Math.round(point.ping_ms)} мс`;
}

// lost requests are not visible in the delay, so they are named
function detailText(point) {
  if (point.score == null) return point.error || 'не отвечает';
  return point.loss > 0 ? `потери ${Math.round(point.loss * 100)}%` : '';
}

// all bars share one scale, so the ways can be compared by eye; it starts near the fastest answer
// instead of zero, otherwise ways that differ by a few tens of ms would look the same
const rangeScale = computed(() => {
  const points = entryPoints.probe?.points ?? [];
  const lows = points.map((p) => p.min_ms ?? p.ping_ms).filter((v) => v != null);
  const highs = points.map((p) => p.max_ms ?? p.ping_ms).filter((v) => v != null);
  const lo = lows.length ? Math.floor(Math.min(...lows) * 0.8) : 0;
  const hi = highs.length ? Math.max(Math.max(...highs) * 1.05, lo + 40) : lo + 40;
  return { lo, span: hi - lo };
});

function pct(value) {
  const { lo, span } = rangeScale.value;
  return `${Math.min(100, Math.max(0, ((value - lo) / span) * 100))}%`;
}

function rangeStyle(point) {
  const min = point.min_ms ?? point.ping_ms;
  const max = point.max_ms ?? point.ping_ms;
  if (min == null || max == null) return { opacity: 0 };
  return { left: pct(min), width: `max(4px, calc(${pct(max)} - ${pct(min)}))` };
}

function midStyle(point) {
  return point.ping_ms == null ? { opacity: 0 } : { left: pct(point.ping_ms) };
}

function rangeTitle(point) {
  if (point.min_ms == null || point.max_ms == null) return '';
  return `Быстрее всего ответил за ${Math.round(point.min_ms)} мс, медленнее всего за ${Math.round(point.max_ms)} мс`;
}

function checkedText(probe) {
  if (!probe) return '';
  const minutes = Math.max(0, Math.floor((now.value / 1000 - probe.checked_at) / 60));
  const when = minutes < 1 ? 'Проверено только что' : `Проверено ${minutes} мин назад`;
  // how many checks the verdict rests on
  const runs = Math.max(0, ...probe.points.map((p) => p.runs));
  return `${when} · проверок в истории: ${runs}`;
}

const verdict = computed(() => {
  const probe = entryPoints.probe;
  if (!probe) return null;
  const best = probe.points.find((p) => p.id === probe.best);
  if (!best) return { text: 'Ни один путь не ответил', level: 'bad' };
  const ms = best.ping_ms == null ? '' : `, ${Math.round(best.ping_ms)} мс`;
  if (probe.preliminary) return { text: `Пока мало данных, проверьте ещё раз позже. Сейчас быстрее всех: ${best.label}${ms}`, level: 'warn' };
  if (probe.suggestion) return { text: `Рекомендуем: ${best.label}${ms}`, level: 'good' };
  if (best.id === 'direct') return { text: `Лучший путь сейчас: напрямую${ms}`, level: 'good' };
  return { text: `Прямое подключение подходит, разница небольшая. Быстрее всех: ${best.label}${ms}`, level: 'good' };
});

const footerText = computed(() => {
  if (!entryPoints.checking) return checkedText(entryPoints.probe);
  const { done, total } = entryPoints.progress;
  const what = entryPoints.mode === 'deep' ? 'Подробная проверка' : 'Проверяю';
  return total ? `${what}: ${Math.min(done, total)} из ${total}` : `${what}...`;
});

// a bar that fills as the requests of the running check come back
const progressStyle = computed(() => {
  const { done, total } = entryPoints.progress;
  return total ? { '--p': `${Math.min(100, (done / total) * 100)}%` } : {};
});
const progressKnown = computed(() => entryPoints.progress.total > 0);
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="open" class="overlay" @click.self="close()">
        <div class="entry-modal" :class="{ shaking }" role="dialog" aria-modal="true" aria-labelledby="entry-modal-title">
          <div class="progress" :class="{ on: entryPoints.checking, known: entryPoints.checking && progressKnown }" :style="progressStyle"></div>
          <button class="close-btn" aria-label="Закрыть" @click="close(true)">
            <svg viewBox="0 0 10 10" width="11" height="11"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3" /></svg>
          </button>
          <h2 id="entry-modal-title">Подбор подключения</h2>
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
                  <span class="point-host">
                    {{ point.host }}<template v-if="detailText(point)"> · {{ detailText(point) }}</template>
                    <button class="copy-host" title="Скопировать адрес" aria-label="Скопировать адрес" @click="copyHost(point.host)">
                      <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg>
                    </button>
                  </span>
                </div>
                <div class="range" :title="rangeTitle(point)">
                  <div class="range-track">
                    <span class="range-fill" :class="entryState(point).level" :style="rangeStyle(point)"></span>
                    <span class="range-mid" :style="midStyle(point)"></span>
                  </div>
                  <span class="range-text">{{ point.min_ms == null ? '' : `${Math.round(point.min_ms)}–${Math.round(point.max_ms)}` }}</span>
                </div>
                <Transition name="swap" mode="out-in">
                  <span :key="pingText(point)" class="point-ping">{{ pingText(point) }}</span>
                </Transition>
                <Transition name="swap" mode="out-in">
                  <span :key="entryState(point).text" class="point-state" :class="entryState(point).level" :title="entryState(point).hint">{{ entryState(point).text }}</span>
                </Transition>
              </div>
            </template>
            <template v-else>
              <div v-for="n in 4" :key="n" class="point" :style="{ '--i': n - 1 }">
                <div class="point-text">
                  <span class="bone wide"></span>
                  <span class="bone narrow"></span>
                </div>
                <span class="bone range"></span>
                <span class="bone ping"></span>
                <span class="bone state"></span>
              </div>
            </template>
          </div>

          <p class="legend">
            Полоска показывает самый быстрый и самый медленный ответ, чёрточка в ней - средний. «Колебания» значит, что пинг заметно скачет и в игре возможны рывки, «Бывают скачки» - что он подскакивает изредка. Подробная проверка занимает около 30 секунд и точнее
          </p>

          <div class="footer">
            <Transition name="swap" mode="out-in">
              <span :key="entryPoints.checking ? 'running' : 'idle'" class="checked">{{ footerText }}</span>
            </Transition>
            <div class="buttons">
              <button class="check-btn" :disabled="entryPoints.checking" title="Около 5 секунд" @click="runEntryProbe('quick')">Быстрая проверка</button>
              <button class="check-btn" :disabled="entryPoints.checking" title="Около 30 секунд, точнее" @click="runEntryProbe('deep')">Подробная</button>
            </div>
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
  width: min(600px, 92vw);
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
/* once the number of requests is known the line fills instead of running */
.progress.known::after {
  width: var(--p, 0%);
  animation: none;
  transition: width 0.3s ease;
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
.verdict.warn {
  background: rgba(231, 200, 115, 0.12);
  color: var(--gold);
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

.copy-host {
  display: inline-flex;
  vertical-align: -1px;
  margin-left: 6px;
  padding: 2px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--text-faint);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.copy-host:hover {
  background: var(--surface-2);
  color: var(--text);
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

.range {
  width: 108px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.range-track {
  position: relative;
  height: 6px;
  border-radius: 3px;
  background: var(--surface-2);
}
.point.best .range-track {
  background: var(--surface);
}
.range-fill {
  position: absolute;
  top: 0;
  bottom: 0;
  border-radius: 3px;
  background: var(--text-faint);
  transition: left 0.5s cubic-bezier(0.2, 0.8, 0.2, 1), width 0.5s cubic-bezier(0.2, 0.8, 0.2, 1), background 0.3s ease, opacity 0.3s ease;
}
.range-fill.good { background: var(--good); }
.range-fill.warn { background: var(--gold); }
.range-fill.bad { background: var(--bad); }
.range-mid {
  position: absolute;
  top: -2px;
  bottom: -2px;
  width: 2px;
  margin-left: -1px;
  border-radius: 1px;
  background: var(--text);
  transition: left 0.5s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.3s ease;
}
.range-text {
  min-height: 14px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-faint);
  text-align: center;
}

.legend {
  margin-top: 2px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--text-faint);
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
.bone.range { width: 108px; }
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

.buttons {
  display: flex;
  gap: 8px;
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

.entry-modal.shaking {
  animation: shake 0.42s ease;
}
@keyframes shake {
  0%, 100% { transform: translateX(0); }
  20% { transform: translateX(-7px); }
  45% { transform: translateX(6px); }
  70% { transform: translateX(-3px); }
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
  .entry-modal.shaking,
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
