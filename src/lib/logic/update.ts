/**
 * The update question's own arithmetic. Pure, so the tests can hammer it.
 * tech.md 6.30.
 */

import type { UpdateState } from '$lib/types/generated/UpdateState';
import type { Update } from '$lib/types/generated/Update';

/**
 * The update on offer, or null.
 *
 * `Ready` is the one state that carries a question; `Checking` and
 * `Downloading` are the island staying quiet on purpose, and `Failed` is a
 * line in the log rather than anything on screen. tech.md 6.30.
 */
export function offered(state: UpdateState | null): Update | null {
  if (!state || typeof state === 'string') return null;
  return 'Ready' in state ? state.Ready : null;
}

/**
 * A file size in the units a person reads, one decimal below a hundred.
 *
 * Megabytes as macOS counts them, a thousand to the kilobyte rather than
 * 1024: this number sits next to a download the Finder will also describe,
 * and two numbers for one file that disagree are worse than either.
 */
export function readableSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '';
  const units = ['B', 'kB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  // Whole numbers above a hundred: a tenth of a megabyte is noise beside a
  // file this size, and the extra character costs width the panel does not
  // have.
  const rounded = unit === 0 || value >= 100 ? Math.round(value) : Math.round(value * 10) / 10;
  return `${rounded} ${units[unit]}`;
}
