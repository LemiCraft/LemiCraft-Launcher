import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import { applyTranslucent } from './store/appearance.js'
import { isMac } from './utils/platform.js'

// macOS gets its own traffic lights and rounded window, so the chrome adapts via this class
if (isMac) document.documentElement.classList.add('mac')

// Set by the backend before the page loads, so the first paint is already translucent
if (window.__LC_TRANSLUCENT__) applyTranslucent(true)

createApp(App).mount('#app')
