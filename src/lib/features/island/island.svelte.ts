/**
 * Island state. The reference vertical every later slice copies: bridge in,
 * runes hold what the window renders, Rust decides what the island shows.
 *
 * The view is Rust's, not the frontend's. Nothing here sets it directly; it
 * arrives on `peekle://view` and leaves as a `set_view` intent. tech.md 8.
 */

import { commands, events } from '$lib/bridge';
import { readNotch, type Notch } from '$lib/logic/shape';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { ToastRequest } from '$lib/types/generated/ToastRequest';

export function createIsland(search = '') {
  const notch: Notch = readNotch(search);

  let view = $state<IslandView>('Collapsed');
  let toast = $state<ToastRequest | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(next: ToastRequest) {
    clearTimeout(timer);
    toast = next;
    // Rust collapses the island on the same deadline. Clearing here too keeps
    // the last toast from flashing back when the next one arrives.
    timer = setTimeout(() => {
      toast = null;
    }, next.ttl_ms);
  }

  async function start(): Promise<() => void> {
    const [offToast, offView] = await Promise.all([
      events.onToast(show),
      events.onView((next) => {
        view = next;
      }),
    ]);

    const state = await commands.getState();
    // A late mount must not drop the view Rust already decided on.
    if (state) view = state.view;

    return () => {
      clearTimeout(timer);
      offToast();
      offView();
    };
  }

  return {
    notch,
    get view() {
      return view;
    },
    get toast() {
      return toast;
    },
    show,
    start,
  };
}
