<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import logo from '../assets/logo.png';
import { accountStore } from '../store/account.js';
import { installState, launchGame } from '../store/installProgress.js';
import { useFocusTrap } from '../composables/useFocusTrap.js';
import { TOUR_STEPS, tourState, finishTour, nextTourStep, previousTourStep } from '../store/tour.js';

const emit = defineEmits(['navigate']);

const PAD = 8;
const CARD_WIDTH = 340;
const WELCOME_WIDTH = 400;
const MARGIN = 16;

const step = computed(() => TOUR_STEPS[tourState.step]);
const rect = ref(null);
const viewport = ref({ width: window.innerWidth, height: window.innerHeight });
const visible = computed(() => tourState.open && !tourState.paused);
const loggedIn = computed(() => accountStore.loggedIn);
const isWelcome = computed(() => !!step.value.welcome);
const isLast = computed(() => tourState.step === TOUR_STEPS.length - 1);
const cardWidth = computed(() => (isWelcome.value ? WELCOME_WIDTH : step.value.task ? CARD_WIDTH + 40 : CARD_WIDTH));
const tourEl = ref(null);
useFocusTrap(tourEl, visible, '.next');

function measure() {
  viewport.value = { width: window.innerWidth, height: window.innerHeight };
  const el = step.value.target ? document.querySelector(step.value.target) : null;
  const box = el?.getBoundingClientRect();
  const next = box && box.width > 0 ? { left: box.left, top: box.top, width: box.width, height: box.height } : null;
  const old = rect.value;
  // a change of a fraction of a pixel is not worth moving the light for
  const same = old && next && ['left', 'top', 'width', 'height'].every((key) => Math.abs(old[key] - next[key]) < 0.5);
  if (!same) rect.value = next;
}

// The page of a step comes in with its own animations, and the place moves while they run: the light follows it
// for a while after the step begins, so that it ends on the place and not where the place was
let followFrame = 0;
function follow(milliseconds = 1600) {
  cancelAnimationFrame(followFrame);
  const until = performance.now() + milliseconds;
  const tick = () => {
    measure();
    if (performance.now() < until) followFrame = requestAnimationFrame(tick);
  };
  tick();
}

function goToStep() {
  emit('navigate', step.value.page);
  nextTick(() => follow());
}

watch(() => [tourState.open, tourState.step, tourState.paused], () => visible.value && goToStep(), { immediate: true });

// The step of the login waits for the player: a login that has worked takes the guide to the next step by itself
watch(loggedIn, (now) => {
  if (now && tourState.open && step.value.task === 'login') nextTourStep();
});

const spotlight = computed(() => {
  const r = rect.value;
  if (!r) return { left: `${viewport.value.width / 2}px`, top: `${viewport.value.height / 2}px`, width: '0px', height: '0px' };
  return { left: `${r.left - PAD}px`, top: `${r.top - PAD}px`, width: `${r.width + PAD * 2}px`, height: `${r.height + PAD * 2}px` };
});

const cardStyle = computed(() => {
  const r = rect.value;
  const { width, height } = viewport.value;
  const w = cardWidth.value;
  if (!r || step.value.placement === 'center') {
    return { left: `${width / 2 - w / 2}px`, top: `${height / 2}px`, transform: 'translateY(-50%)' };
  }
  const clampLeft = (value) => Math.max(MARGIN, Math.min(width - w - MARGIN, value));
  switch (step.value.placement) {
    case 'right':
      return { left: `${r.left + r.width + PAD + MARGIN}px`, top: `${Math.max(MARGIN + 90, Math.min(height - MARGIN - 90, r.top + r.height / 2))}px`, transform: 'translateY(-50%)' };
    case 'top':
      return { left: `${clampLeft(r.left)}px`, top: `${r.top - PAD - MARGIN}px`, transform: 'translateY(-100%)' };
    default:
      return { left: `${clampLeft(r.left)}px`, top: `${r.top + r.height + PAD + MARGIN}px`, transform: 'none' };
  }
});

// What the main button does: the steps with a task do the thing, the others go on
const mainButton = computed(() => {
  const task = step.value.task;
  if (isWelcome.value) return { label: 'Показать', run: nextTourStep };
  if (task === 'login') {
    return loggedIn.value
      ? { label: 'Дальше', run: nextTourStep }
      : { label: 'Войти', run: () => { tourState.loginRequests += 1; } };
  }
  if (task === 'skin') {
    // before the dialog has been opened the button opens it, and after it the button goes on
    return !loggedIn.value || tourState.uploadOpened
      ? { label: 'Дальше', run: nextTourStep }
      : { label: 'Загрузить скин', run: () => { tourState.uploadRequests += 1; } };
  }
  if (task === 'play') {
    return loggedIn.value
      ? { label: 'Играть сейчас', run: () => { finishTour(); launchGame(); } }
      : { label: 'Закончить', run: finishTour };
  }
  return { label: isLast.value ? 'Закончить' : 'Дальше', run: nextTourStep };
});

// the menu of the account is open during the step of the login, and shut when it is over
watch(
  () => tourState.open && !tourState.paused && !!step.value.opensAccountMenu,
  (open) => {
    tourState.accountMenu = open;
  },
  { immediate: true },
);

// The lit place of a step with a task takes the clicks of the player (the button that is lit is the one to press);
// everything around it, and the whole screen at the other steps, is shut by the panels below
const passThrough = computed(() => !!step.value.task && !!rect.value);
const shutters = computed(() => {
  const r = rect.value;
  const { width, height } = viewport.value;
  if (!passThrough.value || !r) return [{ left: '0px', top: '0px', width: '100%', height: '100%' }];
  const left = Math.max(0, r.left - PAD);
  const top = Math.max(0, r.top - PAD);
  const right = Math.min(width, r.left + r.width + PAD);
  const bottom = Math.min(height, r.top + r.height + PAD);
  return [
    { left: '0px', top: '0px', width: '100%', height: `${top}px` },
    { left: '0px', top: `${bottom}px`, width: '100%', height: `${Math.max(0, height - bottom)}px` },
    { left: '0px', top: `${top}px`, width: `${left}px`, height: `${bottom - top}px` },
    { left: `${right}px`, top: `${top}px`, width: `${Math.max(0, width - right)}px`, height: `${bottom - top}px` },
  ];
});

// The game started by the lit button of the last step: the guide has done its work
watch(() => installState.isInstalling, (now) => {
  if (now && tourState.open && step.value.task === 'play') finishTour();
});

// the steps with a task can be put off one by one
const canPutOff = computed(() => !!step.value.task && !(step.value.task === 'login' && loggedIn.value));

function onKeydown(event) {
  if (!visible.value) return;
  if (event.key === 'Escape') finishTour();
  else if (event.key === 'ArrowLeft') previousTourStep();
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('resize', measure);
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('resize', measure);
  cancelAnimationFrame(followFrame);
});
</script>

<template>
  <Transition name="tour">
    <div v-if="visible" ref="tourEl" class="tour" role="dialog" aria-modal="true" aria-label="Экскурсия по лаунчеру">
      <div class="spotlight" :class="{ none: !rect }" :style="spotlight"></div>
      <div v-for="(style, i) in shutters" :key="i" class="shutter" :style="style"></div>

      <div class="card" :class="{ welcome: isWelcome }" :style="{ ...cardStyle, width: `${cardWidth}px` }">
        <button class="close" aria-label="Закрыть экскурсию" @click="finishTour">×</button>

        <Transition name="tour-text" mode="out-in">
          <div :key="tourState.step" class="card-body">
            <img v-if="isWelcome" class="welcome-logo" :src="logo" alt="" />
            <h2>{{ step.title }}</h2>
            <p>{{ step.text }}</p>
            <p v-if="step.task === 'login' && loggedIn" class="done-note">Вы уже вошли в аккаунт</p>
            <p v-else-if="step.task === 'skin' && !loggedIn" class="done-note warn">Скины доступны после входа в аккаунт</p>
            <p v-else-if="step.task === 'play' && !loggedIn" class="done-note warn">Для запуска сначала войдите в аккаунт</p>
          </div>
        </Transition>

        <div class="card-foot" :class="{ 'welcome-foot': isWelcome }">
          <span v-if="!isWelcome" class="dots" aria-hidden="true">
            <span v-for="(_, i) in TOUR_STEPS.slice(1)" :key="i" class="dot" :class="{ on: i + 1 === tourState.step }"></span>
          </span>
          <div class="actions">
            <button v-if="isWelcome" class="btn ghost" @click="finishTour">Не сейчас</button>
            <button v-else-if="canPutOff" class="btn ghost put-off" @click="nextTourStep">Пропустить шаг</button>
            <button v-if="tourState.step > 1" class="btn" @click="previousTourStep">Назад</button>
            <button class="btn primary next" @click="mainButton.run">{{ mainButton.label }}</button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.tour {
  position: fixed;
  inset: 0;
  z-index: 200;
  /* the shutters and the card take the clicks, the lit place lets them through */
  pointer-events: none;
}
.shutter {
  position: absolute;
  pointer-events: auto;
}
.card {
  pointer-events: auto;
}

/* the dimming is the shadow of the lit place: the place stays clear, and it moves from step to step */
.spotlight {
  position: absolute;
  border-radius: 14px;
  box-shadow: 0 0 0 100vmax rgba(6, 6, 9, 0.74), 0 0 0 2px rgba(255, 95, 95, 0.55);
  transition: left 0.4s cubic-bezier(0.2, 0.7, 0.2, 1), top 0.4s cubic-bezier(0.2, 0.7, 0.2, 1),
    width 0.4s cubic-bezier(0.2, 0.7, 0.2, 1), height 0.4s cubic-bezier(0.2, 0.7, 0.2, 1);
  pointer-events: none;
}
.spotlight.none {
  box-shadow: 0 0 0 100vmax rgba(6, 6, 9, 0.74);
}

.card {
  position: absolute;
  box-sizing: border-box;
  padding: 18px 18px 14px;
  border-radius: 16px;
  background: #1d1c22;
  border: 1px solid var(--border);
  box-shadow: var(--shadow-pop);
  transition: left 0.4s cubic-bezier(0.2, 0.7, 0.2, 1), top 0.4s cubic-bezier(0.2, 0.7, 0.2, 1);
}
.close {
  position: absolute;
  top: 9px;
  right: 9px;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--text-faint);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.close:hover {
  background: var(--surface-2);
  color: var(--text);
}
.card-body {
  min-height: 78px;
  padding-right: 14px;
}
/* the greeting: a larger card with the logo, the buttons in the middle */
.card.welcome {
  padding: 30px 30px 24px;
  text-align: center;
}
.card.welcome .card-body {
  min-height: 0;
  padding-right: 0;
}
.welcome-logo {
  width: 64px;
  height: 64px;
  margin-bottom: 14px;
  border-radius: 18px;
  box-shadow: 0 0 40px rgba(255, 95, 95, 0.28);
}
.card.welcome h2 {
  font-size: 21px;
}
.card.welcome p {
  font-size: 14px;
}
.welcome-foot {
  margin-top: 24px;
}
.welcome-foot .actions {
  justify-content: center;
  gap: 10px;
}

.card h2 {
  margin-bottom: 6px;
  font-size: 16px;
  font-weight: 800;
  color: var(--text);
}
.card p {
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-muted);
}
.done-note {
  margin-top: 8px;
  font-weight: 600;
  color: var(--good) !important;
}
.done-note.warn {
  color: var(--gold) !important;
}

.card-foot {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-top: 16px;
}
.actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}
/* the quiet button stays at the left edge, the two others are a pair at the right */
.put-off {
  margin-right: auto;
  padding-left: 0;
}
.dots {
  display: flex;
  gap: 5px;
}
.dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--surface-2);
  transition: background 0.25s ease, width 0.25s ease;
}
.dot.on {
  width: 14px;
  border-radius: 3px;
  background: var(--accent);
}
/* the three kinds of button of the guide: the main one, the usual one, the quiet one; all of one size */
.btn {
  min-height: 36px;
  padding: 0 16px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
}
.btn:hover {
  background: var(--surface-2);
}
.btn.ghost {
  border-color: transparent;
  color: var(--text-muted);
}
.btn.ghost:hover {
  color: var(--text);
}
.btn.primary {
  border-color: transparent;
  background: var(--accent);
  color: var(--accent-text-on);
}
.btn.primary:hover {
  background: var(--accent-hover);
}
.welcome-foot .btn {
  min-width: 132px;
  min-height: 42px;
  font-size: 14px;
}

.tour-enter-active,
.tour-leave-active {
  transition: opacity 0.3s ease;
}
.tour-enter-from,
.tour-leave-to {
  opacity: 0;
}
.tour-text-enter-active {
  transition: opacity 0.22s ease, transform 0.26s cubic-bezier(0.2, 0.7, 0.2, 1);
}
.tour-text-leave-active {
  transition: opacity 0.1s ease;
}
.tour-text-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.tour-text-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .spotlight,
  .card,
  .dot,
  .tour-enter-active,
  .tour-leave-active,
  .tour-text-enter-active,
  .tour-text-leave-active {
    transition: none;
  }
}
</style>
