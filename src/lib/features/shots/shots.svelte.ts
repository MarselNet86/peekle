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
import { secondsLeft, timeLeft } from '$lib/logic/shots';
import type { ShotOffer } from '$lib/types/generated/ShotOffer';

export function createShots() {
  let offer = $state<ShotOffer | null>(null);
  let left = $state(1);
  let secs = $state(0);
  let attached = $state<Record<string, string[]>>({});
  let ticker: ReturnType<typeof setInterval> | undefined;

  /** How often the bar and the number are redrawn while the offer stands.
   *
   * Short enough that five seconds read as a slide rather than as fifty steps
   * (tech.md 6.13), and a timer rather than an animation frame: the Windows
   * webview stops handing frames out, and a countdown that freezes says the
   * offer is still there long after it is gone. tech.md 6.27. */
  const DRAW_MS = 20;

  function stop() {
    if (ticker) clearInterval(ticker);
    ticker = undefined;
  }

  function show(next: ShotOffer | null) {
    stop();
    offer = next;
    if (!next) return;

    // Rust settles the offer on the same deadline and says so with an event.
    // The bar and the number are only the picture of that clock, so they never
    // settle anything themselves: two owners of one deadline disagree the
    // moment either drifts.
    const draw = () => {
      const now = Date.now();
      left = timeLeft(next, now);
      secs = secondsLeft(next, now);
    };
    draw();
    ticker = setInterval(draw, DRAW_MS);
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
      stop();
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
    /** Whole seconds left on the offer. Zero once there is nothing standing. */
    get secs() {
      return secs;
    },
    /** What is waiting in the field of this session. */
    of(sessionId: string): string[] {
      return attached[sessionId] ?? [];
    },
    /**
     * A file chosen from disk. Nothing is written and nothing is read: the
     * file is already there, and the path is the whole of what the message
     * carries. The same path twice is one attachment. tech.md 6.25.
     */
    attach(sessionId: string, path: string) {
      const have = attached[sessionId] ?? [];
      if (have.includes(path)) return;
      attached = { ...attached, [sessionId]: [...have, path] };
    },
    /** The user took one back off the message before sending it. */
    remove(sessionId: string, path: string) {
      attached = {
        ...attached,
        [sessionId]: (attached[sessionId] ?? []).filter((each) => each !== path),
      };
    },
    /**
     * ⌘V with a picture on the clipboard. Rust reads it, writes the file and
     * raises `shot-attached`, so the attachment arrives here by the same
     * route the attach key's does and nothing is added twice. A pasteboard
     * with no picture on it answers null and nothing happens. tech.md 6.13.
     */
    async paste(sessionId: string) {
      try {
        await commands.pasteShot(sessionId);
      } catch (err) {
        // Nowhere to save it is worth a line in the console and no more: the
        // reply being typed is not disturbed by a paste that did not land.
        console.warn('could not paste the screenshot', err);
      }
    },
    /** The message went, so the field is empty again. */
    clear(sessionId: string) {
      attached = { ...attached, [sessionId]: [] };
    },
    start,
  };
}
