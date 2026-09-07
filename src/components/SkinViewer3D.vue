<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { SkinViewer, IdleAnimation } from 'skinview3d';

const props = defineProps({
  url: { type: String, required: true },
  width: { type: Number, default: 150 },
  height: { type: Number, default: 210 },
});

const canvasEl = ref(null);
let viewer = null;

onMounted(() => {
  viewer = new SkinViewer({
    canvas: canvasEl.value,
    width: props.width,
    height: props.height,
    skin: props.url,
    zoom: 0.65,
    fov: 35,
    animation: new IdleAnimation(),
  });
  viewer.autoRotate = true;
  viewer.autoRotateSpeed = 0.5;
  viewer.controls.enableZoom = false;
});

watch(
  () => props.url,
  (url) => viewer?.loadSkin(url),
);

onBeforeUnmount(() => viewer?.dispose());
</script>

<template>
  <canvas ref="canvasEl" class="skin-canvas"></canvas>
</template>

<style scoped>
.skin-canvas {
  display: block;
  cursor: grab;
}
.skin-canvas:active {
  cursor: grabbing;
}
</style>
