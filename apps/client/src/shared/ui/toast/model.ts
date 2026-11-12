import { createStore } from '@/shared/lib';

const VISIBLE_MS = 2200;

export const toastStore = createStore({ message: '', visible: false });
let hideTimer: ReturnType<typeof setTimeout> | undefined;

/** Shows a short status message at the bottom of the stage. */
export function notify(message: string): void {
  toastStore.set({ message, visible: true });
  clearTimeout(hideTimer);
  hideTimer = setTimeout(() => toastStore.set((toast) => ({ ...toast, visible: false })), VISIBLE_MS);
}
