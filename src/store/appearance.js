// Backdrop translucency is only real while the backend has the OS blur applied; the class just lets the page's own backgrounds show it
export function applyTranslucent(on, { animate = false } = {}) {
  const root = document.documentElement;
  // Stays on after the first toggle; kept off for the startup state so it doesn't fade in from opaque
  if (animate) root.classList.add('theme-anim');
  root.classList.toggle('translucent', on);
}
