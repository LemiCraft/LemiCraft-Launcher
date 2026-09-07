import { reactive } from 'vue';

export const confirmDialogState = reactive({
  open: false,
  title: '',
  message: '',
  buttons: [],
});

let resolver = null;

// Resolves with the clicked button's `value`, or null if dismissed (backdrop/✕/Escape)
export function showConfirmDialog({ title, message, buttons }) {
  if (resolver) resolver(null);
  confirmDialogState.title = title;
  confirmDialogState.message = message;
  confirmDialogState.buttons = buttons;
  confirmDialogState.open = true;
  return new Promise((resolve) => {
    resolver = resolve;
  });
}

export function resolveConfirmDialog(value) {
  confirmDialogState.open = false;
  if (resolver) {
    resolver(value);
    resolver = null;
  }
}
