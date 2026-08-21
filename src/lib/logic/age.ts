/** How long ago something happened, in the shortest true form. Pure. */

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

  const elapsed = now - at;
  if (elapsed < MINUTE) return 'now';
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)}m`;
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h`;
  if (elapsed < YEAR) return `${Math.floor(elapsed / DAY)}d`;
  return capped(Math.floor(elapsed / YEAR), 'y');
}

function capped(value: number, unit: string): string {
  return value > CAP ? `${CAP}${unit}+` : `${value}${unit}`;
}
