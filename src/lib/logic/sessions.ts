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
 * The list in the order it was already being read, not the order the data is
 * in. tech.md 6.12.
 *
 * Cards arrive sorted by activity, which is right while there is one chat and
 * wrong the moment there are several: a turn in a chat nobody is looking at
 * lifts it to the top and pushes every row under it down, so a press lands on
 * the row that took the place of the one aimed at. `order` is the sequence of
 * ids the list opened with; anything in it keeps its place, and a chat the
 * list has not seen before goes to the top, where a new chat belongs.
 */
export function steadyOrder(cards: SessionCard[], order: string[]): SessionCard[] {
  if (order.length === 0) return cards;

  const place = new Map(order.map((id, at) => [id, at]));
  const known: SessionCard[] = [];
  const fresh: SessionCard[] = [];

  for (const card of cards) {
    (place.has(card.session.session_id) ? known : fresh).push(card);
  }

  known.sort((a, b) => place.get(a.session.session_id)! - place.get(b.session.session_id)!);
  // New chats first, in the order the data gave them, which is newest first.
  return [...fresh, ...known];
}

/**
 * Whether this chat takes words through the continue path.
 *
 * Every chat the island knows, which since v66 is every chat: one that
 * nobody holds is resumed under its own id, one a live process holds takes
 * the words through that process's inbox, and one that is held and takes
 * nothing is copied. Which of the three is Rust's call at the instant of
 * sending, never a flag baked into the card here -- that read is a filesystem
 * stat with a shelf life of seconds, and a field disabled on a stale one is a
 * field that lies. The only chat that does not come this way is one the
 * island already owns and still holds the process of: it has a pty to write
 * into. tech.md 6.5.
 */
export function canContinue(card: SessionCard | undefined): boolean {
  return card !== undefined;
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

/** The words for a chat that carried on in a copy, because the one it came
 * from is held by another app and takes no messages. Said once, where the
 * conversation now is: the id changed, and a person who is not told reads it
 * as the island having lost their chat. tech.md 6.5. */
export const FORKED_NOTE = {
  fact: 'Another app is holding that chat, so this is a copy of it.',
  how: 'Everything said so far came along. The original stays open where it was.',
};

/**
 * Whether the `Stop` button stands under the field.
 *
 * Only while the agent is mid turn: there is nothing to stop otherwise, and a
 * button that does nothing reads as broken. Never over an open request --
 * that is Allow and Deny's row, and a deny puts the session back to Working,
 * where `Stop` then appears. And only where the field itself is reachable: a
 * session with no process and no pty has no turn to end. tech.md 6.5.
 */
export function stopAvailable(state: {
  status: 'Working' | 'Idle' | 'Ended' | undefined;
  hasPrompt: boolean;
  owned: boolean;
  canContinue: boolean;
  /** When the island already asked this chat to stop. A second press is a
   * second message and a second turn in somebody's chat, so the button takes
   * one press and waits. tech.md 6.5. */
  asked?: number | null;
}): boolean {
  if (state.asked != null) return false;
  return state.status === 'Working' && !state.hasPrompt && (state.owned || state.canContinue);
}

/** Why a second press sends nothing. A press answered by silence reads as a
 * broken button, and a press answered by a second message starts a second
 * turn in somebody else's chat. tech.md 6.5. */
export const STOP_ASKED_NOTE = {
  fact: 'Peekle has already asked this chat to stop.',
  how: 'It runs in another app, so the request waits until the agent reads it. Asking again would only queue a second message.',
};

/** The one word the work line says while a stop request stands.
 *
 * Not `Stopping`: the agent may finish its tool call first, or ignore the
 * request altogether, and the island does not make promises it cannot keep.
 * That it was asked stays true either way. tech.md 6.5 and 6.12. */
export const ASKED_TO_STOP = 'Asked to stop';

/** What one attempt at continuing a chat came back with. */
export type ContinueOutcome =
  | { ok: true; sessionId: string }
  // `error` is null for "there is no Tauri to answer" (dev, Playwright),
  // where nothing surfaces a message about a backend that was never expected
  // to exist -- every other command in the island route stays quiet there
  // too.
  | { ok: false; error: string | null };

/**
 * Turns one `continue_session` attempt into an outcome the caller can act on.
 *
 * Two outcomes now, not three: the words reached a chat, or they did not.
 * "Busy elsewhere" was the third until v66, and it is gone because nothing
 * refuses any more -- a chat that takes no messages is copied rather than
 * declined. The id that comes back is the chat the words actually landed in,
 * which is the same one for every route but the copy. tech.md 6.5.
 */
export function classifyContinueOutcome(
  session: { session_id: string } | null | undefined,
  error: unknown,
): ContinueOutcome {
  if (error !== undefined) return { ok: false, error: String(error) };
  return session ? { ok: true, sessionId: session.session_id } : { ok: false, error: null };
}

/**
 * Whether a chat the user opened is replaced by the way back into the account.
 *
 * An agent needs the network and the account the bars need, so a chat opened
 * while either is gone is a field that takes words and delivers none. Worse
 * than useless: sending into an observed chat with no network came back as
 * "that chat is busy elsewhere", which names the wrong problem entirely and
 * sends the reader looking for another client to close. The screen says what
 * is actually wrong instead.
 *
 * Never while a hook is waiting: answering a live permission request is the
 * one thing the island exists for, and it must work whatever the usage
 * endpoint says -- which is also why the session list itself is never taken
 * away over this. tech.md 6.4 and 6.16.
 */
export function barred(state: { outOfReach: boolean; hasPrompt: boolean }): boolean {
  return state.outOfReach && !state.hasPrompt;
}
