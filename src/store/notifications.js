import { reactive } from 'vue';

export const notifications = reactive([]);

let nextId = 1;
const AUTO_DISMISS_MS = 2500;

export function pushNotification(text, kind = 'success') {
  const id = nextId++;
  notifications.push({ id, text, kind });
  setTimeout(() => dismissNotification(id), AUTO_DISMISS_MS);
}

export function dismissNotification(id) {
  const index = notifications.findIndex((n) => n.id === id);
  if (index !== -1) notifications.splice(index, 1);
}
