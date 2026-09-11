/**
 * What the resting mark says about the agent. Pure, so the property test can
 * hammer it with any list of cards.
 *
 * The mark is the only thing on screen while the island rests, so it carries
 * the one bit the user acts on: whether anything is waiting on them.
 */

import type { IslandView } from '$lib/types/generated/IslandView';
import type { SessionCard } from '$lib/types/generated/SessionCard';

export type RestStatus = 'idle' | 'working' | 'waiting' | 'compacting';

/**
 * Waiting outranks compacting, compacting outranks working, and working
 * outranks everything else. A session that ended or went idle says nothing:
 * Peekle is running either way.
 *
 * Only a permission request makes the mark wait now. A finished turn no longer
 * needs anybody: it holds its channel open by itself and takes what is typed
 * whenever it is typed, so pulsing at the user would be asking for something
 * that is not required. tech.md 6.5 and 6.7.
 *
 * A compact stands above ordinary work because it is not ordinary work: it
 * takes minutes rather than seconds, the agent answers nothing while it runs,
 * and the spinner of a turn over it would say the usual thing is happening.
 * tech.md 6.21.
 */
export function restStatus(cards: SessionCard[], awaitingPermission = false): RestStatus {
  if (awaitingPermission) return 'waiting';
  if (cards.some((card) => card.compacting !== null)) return 'compacting';
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
export function clickPutsAway(
  view: IslandView,
  target: EventTarget | null,
  pressed: EventTarget | null = null,
): boolean {
  if (view === 'Collapsed') return false;
  // A press that began on the shape is not a click beside it, wherever it was
  // let go. That is how text ending at the edge gets selected, and the
  // `click` for it lands on the common ancestor -- the document -- which has
  // no `.shape` above it. tech.md 6.7.
  if (onIsland(pressed)) return false;
  return !onIsland(target);
}

/**
 * Whether a node is the island's own content.
 *
 * A node the island removed on this very click counts. Svelte applies state
 * synchronously after a delegated handler, so a button that deletes itself --
 * the cross on an attachment -- reaches the window already detached, and a
 * detached node has no ancestors at all, `.shape` among them. That is a click
 * on the island's own content, not beside it. tech.md 6.7.
 */
function onIsland(node: EventTarget | null): boolean {
  if (!(node instanceof Element)) return false;
  if (!node.isConnected) return true;
  return node.closest('.shape') !== null;
}

/**
 * What one click beside the shape settles, in order. tech.md 6.13.
 *
 * A screenshot open at full size takes it first. The click is aimed at the
 * picture, not at the island, and collapsing would carry off the feed and the
 * reply being typed along with it. The next such click puts the island away as
 * it always did.
 */
export function clickSettles(
  view: IslandView,
  target: EventTarget | null,
  previewOpen: boolean,
  pressed: EventTarget | null = null,
): 'nothing' | 'preview' | 'island' {
  if (!clickPutsAway(view, target, pressed)) return 'nothing';
  return previewOpen ? 'preview' : 'island';
}
