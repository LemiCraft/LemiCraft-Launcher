import { nextTick, onBeforeUnmount, watch } from 'vue';

const FOCUSABLE = 'button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Keeps the keyboard inside a dialog while it is open: Tab goes round its controls instead of out to the page behind,
// the first control takes the focus on opening, and what had the focus before gets it back on closing.
// `container` is a ref to the element of the dialog, `active` a ref to whether it is open.
export function useFocusTrap(container, active, initialSelector = null) {
  let previous = null;

  function controls() {
    const root = container.value;
    if (!root) return [];
    return [...root.querySelectorAll(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);
  }

  function onKeydown(event) {
    if (event.key !== 'Tab') return;
    const items = controls();
    if (items.length === 0) {
      event.preventDefault();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const inside = container.value?.contains(document.activeElement);
    if (event.shiftKey && (document.activeElement === first || !inside)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (document.activeElement === last || !inside)) {
      event.preventDefault();
      first.focus();
    }
  }

  function release() {
    document.removeEventListener('keydown', onKeydown, true);
    if (previous && typeof previous.focus === 'function') previous.focus({ preventScroll: true });
    previous = null;
  }

  watch(
    active,
    async (isActive) => {
      if (isActive) {
        previous = document.activeElement;
        document.addEventListener('keydown', onKeydown, true);
        await nextTick();
        // focusVisible: false, so that a dialog opened with the mouse does not draw the keyboard ring on its first control
        const initial = initialSelector ? container.value?.querySelector(initialSelector) : null;
        (initial ?? controls()[0])?.focus({ preventScroll: true, focusVisible: false });
      } else if (previous) {
        release();
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(release);
}
