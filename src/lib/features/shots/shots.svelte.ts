/**
 * Screenshot state. Copies the island layout: bridge in, runes hold what the
 * window renders, Rust owns the offer.
 *
 * Two different things live here and they settle differently. The offer is
 * Rust's: it goes up on a pasteboard write, comes down on the key or on its
 * own clock, and nothing here decides either. The attachment is the field's:
 * once a shot is saved it waits beside what the user is typing, and it leaves
 * when the message goes. tech.md 6.13.
 */

import { commands, events } from '$lib/bridge';
import { timeLeft } from '$lib/logic/shots';
import type { ShotOffer } from '$lib/types/generated/ShotOffer';

/** How often the fuse on the offer is redrawn. Five seconds of bar. */
const TICK_MS = 100;

export function createShots() {
  let offer = $state<ShotOffer | null>(null);
  let left = $state(1);
  let attached = $state<Record<string, string[]>>({});
  let timer: ReturnType<typeof setInterval> | undefined;

  function show(next: ShotOffer | null) {
    clearInterval(timer);
    offer = next;
    if (!next) return;

    left = timeLeft(next, Date.now());
    // Rust settles the offer on the same deadline and says so with an event.
    // The bar is only the picture of that clock, so it never settles anything
    // itself: two owners of one deadline disagree the moment either drifts.
    timer = setInterval(() => {
      left = timeLeft(next, Date.now());
    }, TICK_MS);
  }

  async function start(): Promise<() => void> {
    const [offShot, offAttached] = await Promise.all([
      events.onShot(({ offer: next }) => show(next)),
      events.onShotAttached(({ session_id, path }) => {
        attached = { ...attached, [session_id]: [...(attached[session_id] ?? []), path] };
      }),
    ]);

    // A late mount must not drop an offer Rust already put up.
    const state = await commands.getState();
    if (state?.shot) show(state.shot);

    return () => {
      clearInterval(timer);
      offShot();
      offAttached();
    };
  }

  return {
    get offer() {
      return offer;
    },
    get left() {
      return left;
    },
    /** What is waiting in the field of this session. */
    of(sessionId: string): string[] {
      return attached[sessionId] ?? [];
    },
    /** The user took one back off the message before sending it. */
    remove(sessionId: string, path: string) {
      attached = {
        ...attached,
        [sessionId]: (attached[sessionId] ?? []).filter((each) => each !== path),
      };
    },
    /** The message went, so the field is empty again. */
    clear(sessionId: string) {
      attached = { ...attached, [sessionId]: [] };
    },
    start,
  };
}
