/**
 * The number that steps out of the resting island on every ten percent of the
 * five hour window. Copies the island layout: bridge in, runes hold what the
 * window renders, Rust owns the setting. tech.md 6.18.
 *
 * Two pieces of state, not one, and that is the whole point. `value` is the
 * number on screen; `wide` is the shape still standing open for it. The
 * number leaves first and the shape follows, because a shape that collapses
 * under its own content reads as an interruption rather than as a movement.
 * tech.md 6.10.
 */

import { commands } from '$lib/bridge';
import { steppedUp } from '$lib/logic/usage';

/** How long the number stands before it goes. */
export const BADGE_HOLD_MS = 3500;

/** How long it takes to go, and how long the shape waits for it. */
export const BADGE_FADE_MS = 180;

/** What the row says under the switch. */
export const BADGE_HINT = 'The island shows it each time the 5h window crosses a ten.';

export function createBadge() {
  let on = $state(true);
  let busy = $state(false);
  let value = $state<number | null>(null);
  let wide = $state(false);

  /** The last reading judged. Not a rune: nothing renders it. */
  let seen: number | null = null;

  /** A percent the switch promised to show, waiting for the island to rest.
   * The badge belongs to a collapsed island, and the switch that asks for it
   * is inside an open one. tech.md 6.18. */
  let pending: number | null = null;
  let hold: ReturnType<typeof setTimeout> | undefined;
  let shut: ReturnType<typeof setTimeout> | undefined;

  function clear() {
    clearTimeout(hold);
    clearTimeout(shut);
    hold = undefined;
    shut = undefined;
  }

  function show(pct: number) {
    clear();
    // The number, not the ten it tripped: the badge answers how much, and the
    // ten is only what made it worth asking. tech.md 6.18.
    value = Math.round(pct);
    wide = true;

    hold = setTimeout(() => {
      value = null;
      shut = setTimeout(() => (wide = false), BADGE_FADE_MS);
    }, BADGE_HOLD_MS);
  }

  return {
    /** Whether the setting is on. tech.md 6.18. */
    get on() {
      return on;
    },
    /** Flicked and waiting on the answer. */
    get busy() {
      return busy;
    },
    /** The number on screen, or null while there is nothing to announce. */
    get value() {
      return value;
    },
    /** Whether the shape still stands open for it. */
    get wide() {
      return wide;
    },

    /** A fresh reading of the five hour window. */
    track(pct: number | null) {
      const stepped = on && steppedUp(seen, pct);
      seen = pct;
      if (stepped && pct !== null) show(pct);
    },

    /**
     * The island has come back to rest, so a preview held for it plays now.
     * Nothing is held but a preview: a ten crossed while the island was open
     * is not announced at all, because the header was showing the same number
     * at the time. tech.md 6.18.
     */
    rest() {
      if (pending === null) return;
      const pct = pending;
      pending = null;
      show(pct);
    },

    /** What the config says, read once when the island starts. */
    async start(): Promise<() => void> {
      on = (await commands.usageBadge()) ?? true;
      return () => clear();
    },

    /**
     * The switch shows what it turned on. The next ten may be an hour away,
     * and a switch whose effect cannot be seen is a switch nobody believes.
     * The same reason `set_notify_enabled` posts a banner. tech.md 6.18.
     */
    async set(next: boolean, pct: number | null): Promise<void> {
      if (busy) return;
      busy = true;
      try {
        await commands.setUsageBadge(next);
        on = next;
        if (!next) {
          clear();
          pending = null;
          value = null;
          wide = false;
          return;
        }
        // Held rather than shown: the switch is inside an open island, and the
        // badge lives on a resting one. It plays the moment the island rests.
        pending = pct;
      } finally {
        busy = false;
      }
    },
  };
}
