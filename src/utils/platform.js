export const isTauri = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
export const isMac = typeof navigator !== 'undefined' && /mac/i.test(navigator.platform);
