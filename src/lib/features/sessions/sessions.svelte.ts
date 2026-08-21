/**
 * Session list navigation. Rust owns the view, so nothing here sets it: a click
 * becomes a `set_view` intent and the answer comes back on `peekle://view`.
 * tech.md 6.5 and 8.
 */

import { commands } from '$lib/bridge';
import type { IslandView } from '$lib/types/generated/IslandView';

export function openSession(sessionId: string) {
  commands.setView({ Session: sessionId });
}

/**
 * The one entry point into the list: the back chevron of an open session, and
 * the resting mark. Rust answers on `peekle://view`, so nothing here assumes
 * the island opened. tech.md 6.5.
 */
export function openList() {
  commands.setView('Sessions');
}

/**
 * The title the user typed, over the one the hooks derived. An empty title
 * hands the name back to the hooks. tech.md 6.5.
 */
export function renameSession(sessionId: string, title: string) {
  commands.renameSession(sessionId, title.trim());
}

/**
 * Puts a session away in the island. The transcript is untouched: it belongs
 * to Claude Code, and the session stays where the user can still find it
 * there. tech.md 11.
 */
export function hideSession(sessionId: string) {
  commands.hideSession(sessionId);
}

/** The session id a view is showing, or undefined for every other view. */
export function sessionOf(view: IslandView): string | undefined {
  return typeof view === 'object' ? view.Session : undefined;
}
