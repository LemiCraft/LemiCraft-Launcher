import { SkinViewer } from 'skinview3d';

// One shared SkinViewer (a WebGL context per card would blow the browser's concurrent-context limit),
// calls serialized through `queue` since they all reuse the one canvas
let viewer = null;
const cache = new Map();
let queue = Promise.resolve();

function getViewer() {
  if (viewer) return viewer;
  const canvas = document.createElement('canvas');
  viewer = new SkinViewer({ canvas, width: 170, height: 240, zoom: 0.65, fov: 35 });
  viewer.autoRotate = false;
  viewer.controls.enableZoom = false;
  viewer.controls.enablePan = false;
  viewer.controls.enableRotate = false;
  return viewer;
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

// A macrotask boundary (not just rAF) so input can flush before the synchronous toDataURL readback
function macrotask() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

export async function toDataUri(url, force = false) {
  if (!window.__TAURI_INTERNALS__) return url;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke('fetch_skin_data_uri', { url, force });
  } catch {
    return url;
  }
}

const fetchCache = new Map();

export function fetchImageDataUri(url, force = false) {
  if (!url) return Promise.resolve(null);
  if (!force && fetchCache.has(url)) return fetchCache.get(url);
  const promise = toDataUri(url, force).catch((err) => {
    console.error('failed to fetch skin thumbnail image:', err);
    return null;
  });
  fetchCache.set(url, promise);
  return promise;
}

export function renderSkinThumbnail(skinUrl, isSlim, force = false) {
  if (!skinUrl) return Promise.resolve(null);
  const key = `${skinUrl}|${isSlim ? 'slim' : 'default'}`;
  if (!force && cache.has(key)) return cache.get(key);

  const promise = queue.then(async () => {
    try {
      const dataUri = await toDataUri(skinUrl, force);
      const v = getViewer();
      await v.loadSkin(dataUri, { model: isSlim ? 'slim' : 'default' });
      // Two real frame waits so the camera has ticked before capture, else it grabs a garbage close-up crop
      await nextFrame();
      await nextFrame();
      await macrotask();
      v.render();
      const result = v.canvas.toDataURL('image/png');
      await macrotask();
      return result;
    } catch (err) {
      console.error('failed to render skin thumbnail:', err);
      return null;
    }
  });
  queue = promise.then(
    () => {},
    () => {},
  );
  cache.set(key, promise);
  return promise;
}
