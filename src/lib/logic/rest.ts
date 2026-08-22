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
 *
 * Only a permission request makes the mark wait now. A finished turn no longer
 * needs anybody: it holds its channel open by itself and takes what is typed
 * whenever it is typed, so pulsing at the user would be asking for something
 * that is not required. tech.md 6.5 and 6.7.
 */
export function restStatus(cards: SessionCard[], awaitingPermission = false): RestStatus {
  if (awaitingPermission) return 'waiting';
  if (cards.some((card) => card.status === 'Working')) return 'working';
  return 'idle';
}

/**
 * Whether a click puts the island away.
 *
 * An open island takes the mouse on the whole 720 by 560 window, so a click
 * beside the shape is already lost to whatever is underneath; spending it on
 * closing is the one useful thing left.
 *
 * A pending request does not stop this. Collapsing is not resolving: the hook
 * stays pending and still ends on an answer, a dismissal or a timeout, so rule
 * 10 holds. Keeping a window over the whole screen after the user asked for it
 * to go is arguing with them. tech.md 6.7.
 */
export function clickPutsAway(view: IslandView, target: EventTarget | null): boolean {
  if (view === 'Collapsed') return false;
  return !(target instanceof Element && target.closest('.shape'));
}
