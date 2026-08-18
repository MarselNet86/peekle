/** Feed windowing. Pure, so the property tests can hammer it. */

import type { TaskItem } from '$lib/types/generated/TaskItem';

/** tech.md 6.8 caps this at 6 no matter what the config says. */
export const MAX_VISIBLE_ROWS = 6;

export interface FeedWindow {
  visible: TaskItem[];
  /** Shown exactly when something is scrolled out of view. */
  showScrollHint: boolean;
  /** The list disappears entirely when there is nothing to show. */
  showList: boolean;
}

export function feedWindow(rows: TaskItem[], limit = MAX_VISIBLE_ROWS): FeedWindow {
  const capped = Math.min(Math.max(Math.trunc(limit) || 0, 0), MAX_VISIBLE_ROWS);
  return {
    visible: rows.slice(0, capped),
    showScrollHint: rows.length > capped,
    showList: rows.length > 0,
  };
}
