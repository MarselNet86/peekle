/**
 * Island state. The reference vertical every later slice copies: bridge in,
 * runes hold what the window renders, Rust decides when the panel appears.
 */

import { events } from '$lib/bridge';
import type { ToastRequest } from '$lib/types/generated/ToastRequest';

export function createIsland() {
  let toast = $state<ToastRequest | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(next: ToastRequest) {
    clearTimeout(timer);
    toast = next;
    // Rust hides the panel on the same deadline. Clearing here too keeps the
    // last toast from flashing back when the next one arrives.
    timer = setTimeout(() => {
      toast = null;
    }, next.ttl_ms);
  }

  async function start(): Promise<() => void> {
    const unlisten = await events.onToast(show);
    return () => {
      clearTimeout(timer);
      unlisten();
    };
  }

  return {
    get toast() {
      return toast;
    },
    show,
    start,
  };
}
