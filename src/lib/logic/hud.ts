/** HUD windowing. Pure, so the property tests can hammer it. */

import type { TaskItem } from '$lib/types/generated/TaskItem';

/** tech.md 6.8 caps this at 6 no matter what the config says. */
export const MAX_VISIBLE_TASKS = 6;

export interface HudWindow {
  visible: TaskItem[];
  /** Shown exactly when something is scrolled out of view. */
  showScrollHint: boolean;
  /** The HUD disappears entirely when there is nothing to show. */
  showHud: boolean;
}

export function hudWindow(tasks: TaskItem[], limit = MAX_VISIBLE_TASKS): HudWindow {
  const capped = Math.min(Math.max(Math.trunc(limit) || 0, 0), MAX_VISIBLE_TASKS);
  return {
    visible: tasks.slice(0, capped),
    showScrollHint: tasks.length > capped,
    showHud: tasks.length > 0,
  };
}
