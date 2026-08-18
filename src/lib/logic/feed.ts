/** Feed windowing. Pure, so the property tests can hammer it. */

/** tech.md 6.8 caps this at 6 no matter what the config says. */
export const MAX_VISIBLE_ROWS = 6;

export interface FeedWindow<T> {
  visible: T[];
  /** Shown exactly when something is scrolled out of view. */
  showScrollHint: boolean;
  /** The list disappears entirely when there is nothing to show. */
  showList: boolean;
}

/**
 * The last rows of a list, capped. The tail rather than the head: the newest
 * activity is the reason the island opened.
 */
export function feedWindow<T>(rows: T[], limit = MAX_VISIBLE_ROWS): FeedWindow<T> {
  const capped = Math.min(Math.max(Math.trunc(limit) || 0, 0), MAX_VISIBLE_ROWS);
  return {
    visible: rows.slice(Math.max(rows.length - capped, 0)),
    showScrollHint: rows.length > capped,
    showList: rows.length > 0,
  };
}
