/**
 * What the resting mark says about the agent. Pure, so the property test can
 * hammer it with any list of cards.
 *
 * The mark is the only thing on screen while the island rests, so it carries
 * the one bit the user acts on: whether anything is waiting on them.
 */

import type { SessionCard } from '$lib/types/generated/SessionCard';

export type RestStatus = 'idle' | 'working' | 'waiting';

/**
 * Waiting outranks working, and working outranks everything else. A session
 * that ended or went idle says nothing: Peekle is running either way.
 */
export function restStatus(cards: SessionCard[]): RestStatus {
  if (cards.some((card) => card.status === 'WaitingOnUser')) return 'waiting';
  if (cards.some((card) => card.status === 'Working')) return 'working';
  return 'idle';
}
