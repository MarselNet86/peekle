/**
 * The switch under the gear: whether a finished turn puts a banner on the
 * screen. Copies the island layout, bridge in and runes holding what the
 * window renders, with Rust owning the fact. tech.md 6.17.
 */

import { commands } from '$lib/bridge';

/** What the row says under the switch, whichever way it stands.
 *
 * macOS keeps its refusal to itself: banners forbidden in system settings are
 * swallowed silently and the app is never told. So the line names where that
 * is checked rather than claiming everything works. tech.md 6.17. */
export const NOTIFY_HINT =
  'macOS decides whether banners appear. Check System Settings › Notifications.';

export function createNotify() {
  let on = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  return {
    get on() {
      return on;
    },
    /** Flicked and waiting on the answer. The switch does not move until the
     * command comes back: moving first would state a fact nobody confirmed. */
    get busy() {
      return busy;
    },
    get error() {
      return error;
    },

    /** What the config says, read once when the list is first opened. */
    async start(): Promise<void> {
      on = (await commands.notifyEnabled()) ?? false;
    },

    async set(next: boolean): Promise<void> {
      if (busy) return;
      busy = true;
      error = null;
      try {
        await commands.setNotifyEnabled(next);
        on = next;
      } catch (err) {
        // The switch stays where it was: what failed is the change, and a
        // switch that moved anyway would be the only record of a change that
        // did not happen.
        error = String(err);
      } finally {
        busy = false;
      }
    },
  };
}
