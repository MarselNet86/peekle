/**
 * Feed state. Copies the island layout: bridge in, runes hold what the window
 * renders, Rust owns the data.
 *
 * The feed is fed by hooks only. Nothing here invents an entry or rebuilds
 * history from the transcript: that file is written asynchronously and lags
 * the live turn. tech.md section 8.
 */

import { commands, events } from '$lib/bridge';
import type { SessionCard } from '$lib/types/generated/SessionCard';

export function createFeed() {
  let sessions = $state<SessionCard[]>([]);

  async function start(): Promise<() => void> {
    // Subscribe first, then ask. An event sent before the subscription landed
    // reaches nobody -- `listen` is async and the backfill emits from a
    // background thread at start -- so the island cannot live on pushes alone.
    // tech.md section 8.
    let pushed = false;
    const off = await events.onSessions((next) => {
      pushed = true;
      sessions = next;
    });

    const snapshot = await commands.getSessions();
    // A push that arrived while this was in flight is newer by definition, and
    // an older snapshot laid over it would undo whatever it carried.
    if (!pushed) sessions = snapshot ?? [];
    return off;
  }

  return {
    get sessions() {
      return sessions;
    },
    /** The card for one session id, or undefined once it falls off the cap. */
    card(sessionId: string): SessionCard | undefined {
      return sessions.find((card) => card.session.session_id === sessionId);
    },
    start,
  };
}
