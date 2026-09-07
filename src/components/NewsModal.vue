<script setup>
import { computed, nextTick, onMounted, ref } from 'vue';
import { parseBlocks, parseInline, replaceDiscordTimestamps } from '../utils/newsMarkdown.js';
import { pushNotification } from '../store/notifications.js';
import InlineTokens from './InlineTokens.vue';

const props = defineProps({
  item: { type: Object, required: true },
});
const emit = defineEmits(['close']);

const scrollEl = ref(null);
const topFaded = ref(false);
const bottomFaded = ref(false);
function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  topFaded.value = el.scrollTop > 4;
  bottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 4;
}
onMounted(() => nextTick(onScroll));

const blocks = computed(() => parseBlocks(props.item.content, props.item.title).map((b) => ({
  ...b,
  lines: b.lines?.map(parseInline),
  inline: b.text != null ? parseInline(b.text) : undefined,
})));

const titleParts = computed(() => parseInline(replaceDiscordTimestamps(props.item.title || '')));

const fullDate = computed(() => {
  const d = new Date(props.item.rawDate || props.item.date);
  if (Number.isNaN(d.getTime())) return props.item.date;
  return d.toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' });
});

async function openExternal(target) {
  if (!window.__TAURI_INTERNALS__ || !target) return;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_external', { target });
  } catch {
    pushNotification('Не удалось открыть ссылку', 'error');
  }
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="modal">
      <div class="fade fade-top" :class="{ show: topFaded }"></div>

      <div class="modal-scroll" ref="scrollEl" @scroll="onScroll">
        <div v-if="item.imageUrl" class="cover">
          <img :src="item.imageUrl" alt="" />
        </div>

        <div class="body">
          <div class="top-row">
            <span class="tag" :data-tag="item.tag">{{ item.tag }}</span>
            <span class="date">{{ fullDate }}</span>
          </div>

          <h1>
            <template v-for="(t, i) in titleParts" :key="i">
              <a v-if="t.kind === 'url'" href="#" class="title-link" @click.prevent="openExternal(t.href)">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M14 3h7v7h-2V6.4l-9.3 9.3-1.4-1.4L17.6 5H14V3ZM5 5h6v2H5v12h12v-6h2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z"/></svg>
                {{ t.text }}
              </a>
              <span v-else-if="t.kind === 'mention'" class="mention">{{ t.text }}</span>
              <template v-else>{{ t.text }}</template>
            </template>
          </h1>

          <div class="author-row">
            <div class="avatar" v-if="item.authorAvatarUrl">
              <img :src="item.authorAvatarUrl" alt="" />
            </div>
            <div class="avatar avatar-fallback" v-else>{{ (item.author || '?').charAt(0) }}</div>
            <div class="author-text">
              <span class="author-name">{{ item.author }}</span>
              <span class="author-role">{{ item.authorRole }}</span>
            </div>
          </div>

          <div class="content">
            <template v-for="(block, bi) in blocks" :key="bi">
              <component :is="`h${Math.min(block.level + 1, 4)}`" v-if="block.type === 'heading'" class="md-heading">
                <InlineTokens :tokens="block.inline" @open-link="openExternal" />
              </component>
              <blockquote v-else-if="block.type === 'quote'" class="md-quote"><InlineTokens :tokens="block.inline" @open-link="openExternal" /></blockquote>
              <pre v-else-if="block.type === 'code'" class="md-code">{{ block.text }}</pre>
              <p v-else class="md-paragraph">
                <template v-for="(line, li) in block.lines" :key="li">
                  <InlineTokens :tokens="line" @open-link="openExternal" />
                  <br v-if="li < block.lines.length - 1" />
                </template>
              </p>
            </template>
          </div>

          <button v-if="item.url" class="open-btn" @click="openExternal(item.url)">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M14 3h7v7h-2V6.4l-9.3 9.3-1.4-1.4L17.6 5H14V3ZM5 5h6v2H5v12h12v-6h2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z"/></svg>
            Открыть источник
          </button>
        </div>
      </div>

      <div class="fade fade-bottom" :class="{ show: bottomFaded }"></div>

      <button class="close-btn" @click="emit('close')">
        <svg viewBox="0 0 10 10" width="12" height="12"><path d="M0.5 0.5 9.5 9.5M9.5 0.5 0.5 9.5" stroke="currentColor" stroke-width="1.3"/></svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  /* No backdrop-filter: blur() — WebView2 can leave a stuck solid layer behind when it animates */
  background: rgba(8, 7, 9, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  animation: overlay-in 0.15s ease;
}

@keyframes overlay-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.modal {
  position: relative;
  width: min(560px, 88vw);
  max-height: 82vh;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
  animation: modal-in 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modal-in {
  from { opacity: 0; transform: scale(0.96) translateY(6px); }
  to { opacity: 1; transform: scale(1) translateY(0); }
}

.overlay.modal-leave-active {
  animation: overlay-out 0.15s ease forwards;
}
.modal-leave-active .modal {
  animation: modal-out 0.15s cubic-bezier(0.4, 0, 1, 1) forwards;
}
@keyframes overlay-out {
  from { opacity: 1; }
  to { opacity: 0; }
}
@keyframes modal-out {
  from { opacity: 1; transform: scale(1) translateY(0); }
  to { opacity: 0; transform: scale(0.96) translateY(6px); }
}

.fade {
  position: absolute;
  left: 1px;
  right: 1px;
  height: 16px;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease;
  z-index: 1;
}
.fade.show {
  opacity: 1;
}
.fade-top {
  top: 1px;
  border-radius: var(--radius-lg) var(--radius-lg) 0 0;
  background: linear-gradient(var(--surface), transparent);
}
.fade-bottom {
  bottom: 1px;
  border-radius: 0 0 var(--radius-lg) var(--radius-lg);
  background: linear-gradient(transparent, var(--surface));
}

.modal-scroll {
  height: 100%;
  max-height: 82vh;
  overflow-y: auto;
  overflow-x: hidden;
  border-radius: var(--radius-lg);
}

.cover {
  width: 100%;
  height: 200px;
  background: var(--surface-2);
}
.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.body {
  padding: 22px 26px 26px;
}

.top-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}

.tag {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--accent);
  background: var(--accent-soft);
  padding: 3px 8px;
  border-radius: 6px;
}
.tag[data-tag='Сервер'] { color: var(--gold); background: var(--gold-soft); }
.tag[data-tag='Комьюнити'] { color: var(--good); background: var(--good-soft); }
.tag[data-tag='Объявление'] { color: var(--info); background: var(--info-soft); }
.tag[data-tag='Ивент'] { color: var(--event); background: var(--event-soft); }

.date {
  font-size: 12.5px;
  color: var(--text-faint);
}

.body h1 {
  font-size: 22px;
  font-weight: 800;
  color: var(--text);
  line-height: 1.3;
  margin-bottom: 16px;
}

.title-link {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-radius: 10px;
  background: var(--surface-2);
  border: 1px solid var(--border);
  color: var(--text);
  font-size: 15px;
  font-weight: 600;
  text-decoration: none;
  transition: background 0.15s ease, border-color 0.15s ease;
}
.title-link:hover {
  background: var(--surface-hover);
  border-color: #423e46;
}
.title-link svg {
  flex-shrink: 0;
  color: var(--text-faint);
}

.mention {
  background: rgba(88, 101, 242, 0.2);
  color: #939dff;
  border-radius: 4px;
  padding: 0 2px;
}

.author-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-bottom: 18px;
  margin-bottom: 18px;
  border-bottom: 1px solid var(--border);
}

.avatar {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--surface-2);
}
.avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.avatar-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  color: var(--text-muted);
}

.author-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.author-name {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
}
.author-role {
  font-size: 11.5px;
  color: var(--text-faint);
}

.content {
  color: var(--text-muted);
  font-size: 14px;
  line-height: 1.65;
}

.md-paragraph {
  margin-bottom: 12px;
  overflow-wrap: anywhere;
}

.md-heading {
  color: var(--text);
  font-weight: 700;
  margin: 18px 0 10px;
}
h2.md-heading { font-size: 19px; }
h3.md-heading { font-size: 16.5px; }
h4.md-heading { font-size: 15px; }

.md-quote {
  margin: 0 0 12px;
  padding: 8px 14px;
  border-left: 3px solid var(--border);
  color: var(--text-faint);
  font-size: 13.5px;
}

.md-code {
  margin: 0 0 12px;
  padding: 12px 14px;
  border-radius: 8px;
  background: #0c0b0e;
  color: #cfcbd4;
  font-family: var(--font-mono);
  font-size: 12.5px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.open-btn {
  margin-top: 6px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease;
}
.open-btn:hover {
  background: var(--surface-hover);
}

.close-btn {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  border: none;
  background: rgba(0, 0, 0, 0.35);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.15s ease;
  z-index: 5;
}
.close-btn:hover {
  background: rgba(0, 0, 0, 0.55);
}
</style>
