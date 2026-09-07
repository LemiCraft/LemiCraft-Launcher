<script setup>
import { computed } from 'vue';

// Crops just the head — front base + hat overlay — out of the 64x64 skin sheet
const props = defineProps({
  url: { type: String, required: true },
  size: { type: Number, default: 80 },
  radius: { type: Number, default: 10 },
});

const sheetPx = computed(() => `${props.size * 8}px`);
const basePos = computed(() => `-${props.size}px -${props.size}px`);
const hatPos = computed(() => `-${props.size * 5}px -${props.size}px`);
</script>

<template>
  <div class="face-crop" :style="{ width: `${size}px`, height: `${size}px`, borderRadius: `${radius}px` }">
    <div
      class="face-layer"
      :style="{ backgroundImage: `url(${url})`, backgroundSize: `${sheetPx} ${sheetPx}`, backgroundPosition: basePos }"
    ></div>
    <div
      class="face-layer"
      :style="{ backgroundImage: `url(${url})`, backgroundSize: `${sheetPx} ${sheetPx}`, backgroundPosition: hatPos }"
    ></div>
  </div>
</template>

<style scoped>
.face-crop {
  position: relative;
  overflow: hidden;
  flex-shrink: 0;
  filter: drop-shadow(0 6px 10px rgba(0, 0, 0, 0.4));
}

.face-layer {
  position: absolute;
  inset: 0;
  background-repeat: no-repeat;
  image-rendering: pixelated;
}
</style>
