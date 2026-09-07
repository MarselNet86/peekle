/** Searching the session list. Pure, so the property tests can hammer it. */

import type { SessionCard } from '$lib/types/generated/SessionCard';

/**
 * The cards a query keeps, in the order they came.
 *
 * Title and project, case folded, substring. Not fuzzy: a list of twenty rows
 * is read, not searched, and fuzzy matching on twenty rows mostly produces
 * surprises. An empty query keeps everything.
 */
export function searchSessions(cards: SessionCard[], query: string): SessionCard[] {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return cards;

  return cards.filter((card) => {
    const haystack = `${card.title} ${card.session.project}`.toLocaleLowerCase();
    return haystack.includes(needle);
  });
}

/**
 * Whether this chat can be forked into one the island owns.
 *
 * Only an observed chat: an owned one already has a field. Whether a live
 * client is writing it right now is not guessed here -- that read is a
 * filesystem stat with a shelf life of seconds, and baking it into every card
 * handed to the island turned into a dead field the moment it went stale. Rust
 * makes the real, fresh check at the instant it actually matters: when a
 * message is about to be sent. tech.md 6.5.
 */
export function canContinue(card: SessionCard | undefined): boolean {
  return card?.origin === 'Observed';
}

/**
 * Whether the reply field takes a press right now.
 *
 * Answering a permission request or writing into a session the island already
 * owns is a fire-and-forget write to a pty that is already running: nothing
 * to wait for, so nothing to guard. Forking an observed chat is the one real
 * round trip -- spawning a process, not writing to one -- and `continuing`
 * covers exactly its width: from the press that starts it to the moment it
 * lands or fails. A second press inside that window used to race a second
 * fork of the same chat, because nothing on screen changed to say the first
 * one was still in flight. tech.md 6.5.
 */
export function replyReachable(state: {
  hasPrompt: boolean;
  owned: boolean;
  canContinue: boolean;
  continuing: boolean;
}): boolean {
  return state.hasPrompt || state.owned || (state.canContinue && !state.continuing);
}
