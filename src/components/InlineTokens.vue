<script setup>
defineProps({ tokens: { type: Array, required: true } });
const emit = defineEmits(['open-link']);

function classesFor(t) {
  return {
    bold: t.bold,
    italic: t.italic,
    strike: t.strike,
    code: t.code,
    spoiler: t.spoiler,
  };
}
</script>

<template>
  <template v-for="(t, i) in tokens" :key="i">
    <a v-if="t.kind === 'url'" href="#" class="link" :class="classesFor(t)" @click.prevent="emit('open-link', t.href)">{{ t.text }}</a>
    <span v-else :class="[t.kind === 'mention' ? 'mention' : null, classesFor(t)]">{{ t.text }}</span>
  </template>
</template>

<style scoped>
.bold { font-weight: 700; }
.italic { font-style: italic; }
.strike { text-decoration: line-through; }
.code {
  font-family: var(--font-mono);
  font-size: 12.5px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--text);
  padding: 1px 5px;
  border-radius: 4px;
}
.spoiler {
  background: rgba(255, 255, 255, 0.16);
  color: transparent;
  border-radius: 4px;
  cursor: pointer;
  transition: color 0.1s ease;
}
.spoiler:hover {
  color: var(--text);
  background: rgba(255, 255, 255, 0.08);
}
.link {
  color: #939dff;
  text-decoration: underline;
}
.mention {
  background: rgba(88, 101, 242, 0.2);
  color: #939dff;
  border-radius: 4px;
  padding: 0 2px;
}
</style>
