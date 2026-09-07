<script setup>
import PlayHero from './PlayHero.vue';
import NewsFeed from './NewsFeed.vue';
import NoModsBanner from './NoModsBanner.vue';

defineEmits(['navigate']);
</script>

<template>
  <div class="home">
    <PlayHero />
    <NoModsBanner @navigate="$emit('navigate', $event)" />
    <NewsFeed />
  </div>
</template>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  gap: 20px;
  height: 100%;
  min-height: 0;
}
</style>
