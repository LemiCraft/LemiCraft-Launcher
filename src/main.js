import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import { applyTranslucent } from './store/appearance.js'

// macOS draws its own traffic lights over the page, so the window chrome adapts via this class
if (/mac/i.test(navigator.platform)) document.documentElement.classList.add('mac')

// Set by the backend before the page loads, so the first paint is already translucent
if (window.__LC_TRANSLUCENT__) applyTranslucent(true)

createApp(App).mount('#app')
