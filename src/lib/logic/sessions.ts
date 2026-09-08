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

/**
 * The exact refusal `continue_session` gives when another client is driving
 * the chat right now. Matched here rather than treated as an ordinary error,
 * so a rejection can become "wait and try again" instead of a dead end.
 * tech.md 6.5.
 */
export const BUSY_ELSEWHERE = 'That chat is open somewhere else right now';

export function isBusyElsewhere(error: unknown): boolean {
  return String(error) === BUSY_ELSEWHERE;
}

/** What one attempt at continuing a chat came back with. */
export type ContinueOutcome =
  | { ok: true; sessionId: string }
  | { ok: false; busy: true }
  // `error` is null for "there is no Tauri to answer" (dev, Playwright),
  // where nothing surfaces a message about a backend that was never expected
  // to exist -- every other command in the island route stays quiet there
  // too.
  | { ok: false; busy: false; error: string | null };

/**
 * Turns one `continue_session` attempt into an outcome the caller can act on
 * without re-deriving the classification: a real session to open, a chat
 * that is busy and worth trying again, or an error, and neither of the last
 * two is confused with the other. Exactly one of `session`/`error` is ever
 * meaningful, matching the one try/catch that produces them. tech.md 6.5.
 */
export function classifyContinueOutcome(
  session: { session_id: string } | null | undefined,
  error: unknown,
): ContinueOutcome {
  if (error !== undefined) {
    return isBusyElsewhere(error)
      ? { ok: false, busy: true }
      : { ok: false, busy: false, error: String(error) };
  }
  return session
    ? { ok: true, sessionId: session.session_id }
    : { ok: false, busy: false, error: null };
}

/**
 * Whether a chat the user opened is replaced by the way back into the account.
 *
 * A conversation the island cannot reach is a dead screen with a scrollbar, so
 * the sign-in stands where the chat would be. Never while a hook is waiting:
 * answering a live permission request is the one thing the island exists for,
 * and it must work whatever the usage endpoint says -- which is also why the
 * session list itself is never taken away over this. tech.md 6.4 and 6.16.
 */
export function barred(state: { needsSignIn: boolean; hasPrompt: boolean }): boolean {
  return state.needsSignIn && !state.hasPrompt;
}
