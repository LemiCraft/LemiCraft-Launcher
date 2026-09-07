export function readImageFile(file) {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => resolve({ img, url, width: img.naturalWidth, height: img.naturalHeight });
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error('Не удалось загрузить изображение'));
    };
    img.src = url;
  });
}

// Ported from the old launcher's UploadSkinWindow.DetectSkinModel: samples the arm-overlay
// region (x 54-56, y 20-32) — mostly-transparent there means a slim (Alex) arm, not classic (Steve)
export function detectSkinModel(img, width, height) {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  ctx.drawImage(img, 0, 0);

  const rw = Math.min(56, width) - 54;
  const rh = Math.min(32, height) - 20;
  if (rw <= 0 || rh <= 0) return 'steve';

  const { data } = ctx.getImageData(54, 20, rw, rh);
  let transparent = 0;
  const total = rw * rh;
  for (let i = 3; i < data.length; i += 4) {
    if (data[i] === 0) transparent++;
  }
  return total > 0 && transparent / total > 0.8 ? 'alex' : 'steve';
}
