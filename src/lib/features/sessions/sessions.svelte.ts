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

export function backToList() {
  commands.setView('Sessions');
}

/** The session id a view is showing, or undefined for every other view. */
export function sessionOf(view: IslandView): string | undefined {
  return typeof view === 'object' ? view.Session : undefined;
}
