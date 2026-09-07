<script setup>
import { computed, onMounted, ref } from 'vue';
import NewsModal from './NewsModal.vue';
import { parseInline, replaceDiscordTimestamps, shortDisplayTitle } from '../utils/newsMarkdown.js';
import { newsState, fetchNews, loadMoreNews } from '../store/news.js';

const scrollEl = ref(null);
const topFaded = ref(false);
const bottomFaded = ref(true);
const openItem = ref(null);
const refreshing = ref(false);
const SKELETON_COUNT = 6;

async function onRefresh() {
  refreshing.value = true;
  try {
    await fetchNews({ force: true });
  } finally {
    refreshing.value = false;
  }
}

const feedState = computed(() => {
  if (newsState.loading && !newsState.loaded) return 'loading';
  if (newsState.error) return 'error';
  if (newsState.loaded && newsState.items.length === 0) return 'empty';
  return 'content';
});

function excerptTokens(text) {
  return parseInline(replaceDiscordTimestamps(text || ''));
}

function cardTitle(title) {
  return title && title.length > 500 ? `${title.slice(0, 500)}...` : title;
}

function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  topFaded.value = el.scrollTop > 4;
  bottomFaded.value = el.scrollTop + el.clientHeight < el.scrollHeight - 4;
  if (el.scrollTop + el.clientHeight > el.scrollHeight - 400) loadMoreNews();
}

function openNews(item) {
  openItem.value = item;
}

onMounted(() => fetchNews());
</script>

<template>
  <section class="feed">
    <div class="feed-head">
      <h2>Новости</h2>
      <button class="refresh" :disabled="refreshing || newsState.loading" @click="onRefresh">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M12 4a8 8 0 1 0 7.75 6h-2.08A6 6 0 1 1 12 6V2l5 4-5 4V6Z"/></svg>
        {{ refreshing || newsState.loading ? 'Обновляю...' : 'Обновить' }}
      </button>
    </div>

    <div class="feed-scroll-wrap">
      <div class="fade fade-top" :class="{ show: topFaded }"></div>

      <div class="feed-scroll" ref="scrollEl" @scroll="onScroll">
        <Transition name="crossfade">
          <div v-if="feedState === 'loading'" key="loading" class="feed-grid">
            <div v-for="i in SKELETON_COUNT" :key="i" class="news-skeleton" :style="{ animationDelay: `${i * 45}ms` }">
              <div class="sk-top">
                <span class="sk-tag"></span>
                <span class="sk-date"></span>
              </div>
              <span class="sk-title"></span>
              <span class="sk-line"></span>
              <span class="sk-line short"></span>
            </div>
          </div>

          <p v-else-if="feedState === 'error'" key="error" class="feed-error">Не удалось загрузить новости. Проверьте подключение и нажмите "Обновить"</p>

          <p v-else-if="feedState === 'empty'" key="empty" class="feed-error">Новостей пока нет</p>

          <TransitionGroup v-else key="content" name="card" tag="div" class="feed-grid">
            <article
              v-for="(item, i) in newsState.items"
              :key="item.id"
              class="news-card clickable"
              :style="{ animationDelay: `${i * 45}ms` }"
              @click="openNews(item)"
            >
              <div class="news-top">
                <span class="tag" :data-tag="item.tag">{{ item.tag }}</span>
                <span class="date">{{ item.date }}</span>
              </div>
              <h3>
                <template v-for="(t, ti) in excerptTokens(shortDisplayTitle(cardTitle(item.title)))" :key="ti">
                  <span v-if="t.kind === 'mention'" class="mention">{{ t.text }}</span>
                  <span v-else-if="t.kind === 'url'" class="link-text">{{ t.text }}</span>
                  <template v-else>{{ t.text }}</template>
                </template>
              </h3>
              <p>
                <template v-for="(t, ti) in excerptTokens(item.excerpt)" :key="ti">
                  <span v-if="t.kind === 'mention'" class="mention">{{ t.text }}</span>
                  <span v-else-if="t.kind === 'url'" class="link-text">{{ t.text }}</span>
                  <template v-else>{{ t.text }}</template>
                </template>
              </p>
            </article>
          </TransitionGroup>
        </Transition>

        <div v-if="feedState === 'content'" class="load-more">
          <span v-if="newsState.loadingMore" class="load-more-spinner"></span>
          <span v-else-if="!newsState.hasMore" class="load-more-end">Это все новости</span>
        </div>
      </div>

      <div class="fade fade-bottom" :class="{ show: bottomFaded }"></div>
    </div>

    <Transition name="modal">
      <NewsModal v-if="openItem" :item="openItem" @close="openItem = null" />
    </Transition>
  </section>
</template>

<style scoped>
.feed {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  flex: 1;
}

.feed-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.feed-head h2 {
  font-size: 18px;
  font-weight: 700;
  color: var(--text);
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
  transition: background 0.15s ease, color 0.15s ease, transform 0.15s ease;
}
.refresh:hover {
  background: var(--surface-2);
  color: var(--text);
}
.refresh:active svg {
  transform: rotate(180deg);
}
.refresh svg {
  transition: transform 0.3s ease;
}

.feed-scroll-wrap {
  position: relative;
  min-height: 0;
  flex: 1;
}

.feed-scroll {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 6px 4px 0 2px;
  position: relative;
}

.feed-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
  padding-bottom: 4px;
}

.feed-error {
  padding: 24px;
  text-align: center;
  color: var(--text-faint);
  font-size: 13.5px;
}

.load-more {
  display: flex;
  justify-content: center;
  padding: 16px 0 4px;
}

.load-more-end {
  font-size: 12px;
  color: var(--text-faint);
}

.load-more-spinner {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid var(--surface-2);
  border-top-color: var(--accent);
  animation: spin 0.7s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
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
  top: 6px;
  left: 4px;
  right: 2px;
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

.mention {
  background: rgba(88, 101, 242, 0.2);
  color: #939dff;
  border-radius: 4px;
  padding: 0 2px;
}

.link-text {
  color: #939dff;
}

@media (prefers-reduced-motion: reduce) {
  .crossfade-enter-active,
  .crossfade-leave-active,
  .card-enter-active,
  .card-leave-active,
  .card-move {
    transition: none;
  }
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

.news-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 16px 18px;
  transition: border-color 0.15s ease, transform 0.15s ease;
  animation: card-in 0.4s cubic-bezier(0.16, 1, 0.3, 1) backwards;
}
.news-card.clickable {
  cursor: pointer;
}
.news-card:hover {
  border-color: #423e46;
  transform: translateY(-2px);
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

.news-skeleton {
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 16px 18px;
  animation: card-in 0.4s cubic-bezier(0.16, 1, 0.3, 1) backwards;
}

.sk-top {
  display: flex;
  align-items: center;
  gap: 10px;
}

.sk-tag,
.sk-date,
.sk-title,
.sk-line {
  display: block;
  border-radius: 6px;
  background: var(--surface-2);
  animation: pulse 1.4s ease-in-out infinite;
}

.sk-tag {
  width: 64px;
  height: 16px;
  border-radius: 6px;
}

.sk-date {
  width: 48px;
  height: 12px;
}

.sk-title {
  width: 80%;
  height: 16px;
  margin-top: 2px;
}

.sk-line {
  width: 100%;
  height: 12px;
}
.sk-line.short {
  width: 60%;
}

.news-top {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
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
.tag[data-tag='Сервер'] {
  color: var(--gold);
  background: var(--gold-soft);
}
.tag[data-tag='Комьюнити'] {
  color: var(--good);
  background: var(--good-soft);
}
.tag[data-tag='Объявление'] {
  color: var(--info);
  background: var(--info-soft);
}
.tag[data-tag='Ивент'] {
  color: var(--event);
  background: var(--event-soft);
}

.date {
  font-size: 12px;
  color: var(--text-faint);
}

.news-card h3 {
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
  margin-bottom: 6px;
  overflow-wrap: anywhere;
}

.news-card p {
  font-size: 13.5px;
  color: var(--text-muted);
  line-height: 1.5;
  overflow-wrap: anywhere;
}

@media (prefers-reduced-motion: reduce) {
  .news-card,
  .news-skeleton {
    animation: none;
  }
  .sk-tag,
  .sk-date,
  .sk-title,
  .sk-line {
    animation: none;
  }
  .load-more-spinner {
    animation: none;
  }
}
</style>
