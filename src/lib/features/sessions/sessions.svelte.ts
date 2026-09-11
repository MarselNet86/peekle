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
 * Deletes a chat for good: its process if the island runs it, its transcript
 * into the macOS Trash, and its row.
 *
 * Answers with why it could not, or null when it did. The row stays and says
 * so in that case: the one thing a delete may never do is look like it
 * happened. tech.md 6.26.
 */
export async function deleteSession(sessionId: string): Promise<string | null> {
  try {
    await commands.deleteSession(sessionId);
    return null;
  } catch (err) {
    return String(err);
  }
}

/** The session id a view is showing, or undefined for every other view. */
export function sessionOf(view: IslandView): string | undefined {
  return typeof view === 'object' ? view.Session : undefined;
}
