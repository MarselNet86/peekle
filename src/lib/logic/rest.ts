/**
 * What the resting mark says about the agent. Pure, so the property test can
 * hammer it with any list of cards.
 *
 * The mark is the only thing on screen while the island rests, so it carries
 * the one bit the user acts on: whether anything is waiting on them.
 */

import type { IslandView } from '$lib/types/generated/IslandView';
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

/**
 * Whether a click puts the island away.
 *
 * An open island takes the mouse on the whole 720 by 560 window, so a click
 * beside the shape is already lost to whatever is underneath; spending it on
 * closing is the one useful thing left. A pending request is left alone: rule
 * 10 wants a blocking hook resolved by an answer, a dismissal or a timeout,
 * never by a stray click. tech.md 6.7.
 */
export function clickPutsAway(
  view: IslandView,
  hasPrompt: boolean,
  target: EventTarget | null,
): boolean {
  if (view === 'Collapsed' || hasPrompt) return false;
  return !(target instanceof Element && target.closest('.shape'));
}
