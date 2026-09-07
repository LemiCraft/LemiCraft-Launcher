import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  // Node/Vite were binding the dev server to ::1 (IPv6) only on this machine, while Tauri's
  // "is the dev server up yet" check resolves localhost to 127.0.0.1 — the two never met
  server: {
    host: '127.0.0.1',
  },
})
