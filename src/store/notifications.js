import { reactive } from 'vue';

export const notifications = reactive([]);

let nextId = 1;
const AUTO_DISMISS_MS = 2500;
// an error is read and often copied, so it stays longer
const PROBLEM_DISMISS_MS = 9000;

export function pushNotification(text, kind = 'success', duration) {
  const id = nextId++;
  notifications.push({ id, text, kind });
  setTimeout(() => dismissNotification(id), duration ?? (kind === 'error' || kind === 'warn' ? PROBLEM_DISMISS_MS : AUTO_DISMISS_MS));
}

export function dismissNotification(id) {
  const index = notifications.findIndex((n) => n.id === id);
  if (index !== -1) notifications.splice(index, 1);
}
