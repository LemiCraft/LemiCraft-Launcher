<script setup>
import { nextTick, onMounted, ref, watch } from 'vue';
import { rulesState, fetchRules } from '../store/rules.js';

onMounted(() => {
  fetchRules();
  nextTick(onScroll);
});

const scrollEl = ref(null);
const topFaded = ref(false);
const bottomFaded = ref(false);
function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  topFaded.value = el.scrollTop > 4;
  bottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 4;
}

watch(
  () => [rulesState.doc, rulesState.pendingAnchors],
  async ([doc, anchors]) => {
    if (!doc || !anchors?.length) return;
    await nextTick();
    document.getElementById(`rule-${anchors[0]}`)?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    rulesState.highlightedRuleIds = anchors;
    rulesState.pendingAnchors = [];
    nextTick(onScroll);
  },
  { immediate: true },
);

watch(
  () => rulesState.doc,
  () => nextTick(onScroll),
);
</script>

<template>
  <div class="rules">
    <div class="head">
      <h1>Правила сервера</h1>
      <p class="sub">Распространяются на все площадки проекта — сервер, Discord и другие ресурсы LemiCraft</p>
    </div>

    <div class="rules-scroll-wrap">
      <div class="fade fade-top" :class="{ show: topFaded }"></div>

      <div class="rules-scroll" ref="scrollEl" @scroll="onScroll">
        <div v-if="rulesState.loading && !rulesState.doc" class="empty-state">Загрузка...</div>
        <div v-else-if="rulesState.error && !rulesState.doc" class="empty-state">Не удалось загрузить правила</div>

        <template v-else-if="rulesState.doc">
          <section v-for="section in rulesState.doc.sections" :key="section.number" class="rule-section">
            <h2>{{ section.emoji }} {{ section.number }}. {{ section.title }}</h2>
            <p
              v-for="rule in section.rules"
              :key="rule.id"
              :id="`rule-${rule.id}`"
              class="rule-item"
              :class="{ highlighted: rulesState.highlightedRuleIds.includes(rule.id) }"
            >
              <span class="anchor-mark">#</span>{{ rule.id }}. {{ rule.text }}
            </p>
            <div v-for="(note, i) in section.notes" :key="i" class="note" :class="note.kind">{{ note.text }}</div>
          </section>

          <div class="footer-block">
            <p v-for="(line, i) in rulesState.doc.footer" :key="i">{{ line }}</p>
            <p class="updated-at">Правила последний раз обновлены: {{ rulesState.doc.updatedAt }}</p>
          </div>
        </template>
      </div>

      <div class="fade fade-bottom" :class="{ show: bottomFaded }"></div>
    </div>
  </div>
</template>

<style scoped>
.rules {
  display: flex;
  flex-direction: column;
  gap: 20px;
  height: 100%;
  min-height: 0;
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

.rules-scroll-wrap {
  position: relative;
  min-height: 0;
  flex: 1;
}

.rules-scroll {
  height: 100%;
  overflow-y: auto;
  padding-right: 4px;
  display: flex;
  flex-direction: column;
  gap: 22px;
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

.empty-state {
  padding: 20px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  color: var(--text-faint);
  font-size: 13.5px;
  text-align: center;
}

.rule-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.rule-section h2 {
  font-size: 14.5px;
  font-weight: 700;
  color: var(--text);
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 2px;
}

.rule-item {
  position: relative;
  font-size: 13px;
  color: var(--text-muted);
  line-height: 1.6;
  scroll-margin-top: 20px;
  padding: 2px 8px 2px 4px;
  border-radius: 6px;
  transition: background-color 0.6s ease;
}
.rule-item.highlighted {
  background: var(--accent-soft);
}

.anchor-mark {
  display: inline-block;
  width: 12px;
  margin-right: 3px;
  color: var(--text-faint);
  opacity: 0;
  transition: opacity 0.15s ease;
  user-select: none;
}
.rule-item:hover .anchor-mark {
  opacity: 1;
}

.note {
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  font-size: 12.5px;
  line-height: 1.5;
}
.note.info {
  background: var(--surface-2);
  color: var(--text-muted);
}
.note.warning {
  border: 1px solid rgba(239, 68, 68, 0.3);
  background: var(--bad-soft);
  color: var(--bad);
}

.footer-block {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text-muted);
  line-height: 1.5;
  flex-shrink: 0;
}

.updated-at {
  color: var(--text-faint);
  margin-top: 4px;
}
</style>
