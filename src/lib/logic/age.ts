/** How long ago something happened, in the shortest true form. Pure. */

import { copy } from '$lib/i18n/index.svelte';
import { LIST } from '$lib/i18n/list';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const YEAR = 365 * DAY;

/** Past this the number is noise, and the row still has a title to fit. */
const CAP = 99;

/**
 * The age of a timestamp as a picker writes it: `now`, `5m`, `3h`, `4d`, `2y`.
 *
 * One unit, never two. This sits at the end of a row next to a title, and a
 * second unit buys precision nobody reads at the cost of the width the title
 * needs. A future timestamp reads as `now` rather than as a negative age:
 * clocks disagree, and no row should ever say `-2m`. The number is capped for
 * the same reason the unit is single: a garbage timestamp must not be able to
 * push the title out of its own row.
 */
export function ageLabel(at: number, now: number): string {
  if (!Number.isFinite(at) || !Number.isFinite(now)) return '';

  const t = copy(LIST);
  const elapsed = now - at;
  if (elapsed < MINUTE) return t.now;
  if (elapsed < HOUR) return t.minutes(Math.floor(elapsed / MINUTE));
  if (elapsed < DAY) return t.hours(Math.floor(elapsed / HOUR));
  if (elapsed < YEAR) return t.days(Math.floor(elapsed / DAY));
  const years = Math.floor(elapsed / YEAR);
  return years > CAP ? t.years(CAP, true) : t.years(years, false);
}
