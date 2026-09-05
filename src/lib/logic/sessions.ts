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
 * Only an observed chat that nobody else is driving. An owned one already has
 * a field; one a live client is writing cannot be resumed at all, because that
 * would put two agents on one branch. Rust reads that fact from the transcript
 * mtime and puts it on the card, so the field can say so before anything is
 * typed rather than refusing on submit. tech.md 6.5.
 */
export function canContinue(card: SessionCard | undefined): boolean {
  return card?.origin === 'Observed' && !card.live_elsewhere;
}
