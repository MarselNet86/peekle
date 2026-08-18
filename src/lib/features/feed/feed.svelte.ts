/**
 * Feed state. Copies the island layout: bridge in, runes hold what the window
 * renders, Rust owns the data.
 *
 * The feed is fed by hooks only. Nothing here invents an entry or rebuilds
 * history from the transcript: that file is written asynchronously and lags
 * the live turn. tech.md section 8.
 */

import { events } from '$lib/bridge';
import type { SessionCard } from '$lib/types/generated/SessionCard';

export function createFeed() {
  let sessions = $state<SessionCard[]>([]);

  async function start(): Promise<() => void> {
    return events.onSessions((next) => {
      sessions = next;
    });
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
