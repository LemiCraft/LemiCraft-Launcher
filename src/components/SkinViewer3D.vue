<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { SkinViewer, IdleAnimation } from 'skinview3d';

const props = defineProps({
  url: { type: String, required: true },
  width: { type: Number, default: 150 },
  height: { type: Number, default: 210 },
  // left out, the library's own proportions are used (the same as on the site)
  zoom: { type: Number, default: undefined },
  fov: { type: Number, default: undefined },
  // the figure breathes and turns by itself; off, it stands still and turns only by hand
  lively: { type: Boolean, default: true },
  // follow the size of the parent instead of width/height
  fit: { type: Boolean, default: false },
});
const emit = defineEmits(['ready']);

const FIT_RATIO = 0.6;
const FIT_MAX_HEIGHT = 400;

const canvasEl = ref(null);
let viewer = null;
let resizeObserver = null;
let loadToken = 0;
let disposed = false;

// A slow load of an old skin must not paint over a newer one, nor run on a viewer that is gone
function loadSkinOf(url) {
  const token = ++loadToken;
  viewer.loadSkin(url).then(
    () => { if (token === loadToken && !disposed) emit('ready'); },
    () => { if (token === loadToken && !disposed) emit('ready'); },
  );
}

onMounted(() => {
  viewer = new SkinViewer({
    canvas: canvasEl.value,
    width: props.width,
    height: props.height,
    ...(props.zoom != null && { zoom: props.zoom }),
    ...(props.fov != null && { fov: props.fov }),
    animation: props.lively ? new IdleAnimation() : null,
  });
  viewer.autoRotate = props.lively;
  viewer.autoRotateSpeed = 0.5;
  viewer.controls.enableZoom = false;
  if (props.fit && canvasEl.value.parentElement) {
    resizeObserver = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      if (!(width > 0 && height > 0)) return;
      // the figure is tall and narrow: the biggest 3:5 box that fits the place, never taller than on the site
      const h = Math.min(height, width / FIT_RATIO, FIT_MAX_HEIGHT);
      viewer.setSize(Math.max(1, Math.floor(h * FIT_RATIO)), Math.max(1, Math.floor(h)));
    });
    resizeObserver.observe(canvasEl.value.parentElement);
  }
  // ready also on a failed load: the viewer shows the default skin then, and the page must not wait for ever
  loadSkinOf(props.url);
});

watch(
  () => props.url,
  (url) => viewer && loadSkinOf(url),
);

onBeforeUnmount(() => {
  disposed = true;
  resizeObserver?.disconnect();
  viewer?.dispose();
  // dispose() leaves the WebGL context to the garbage collector, and a browser keeps only about sixteen
  viewer?.renderer?.forceContextLoss?.();
});
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
