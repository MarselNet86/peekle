/** Usage bar math. Pure, so the property tests can hammer it. */

export type UsageTone = 'accent' | 'warn' | 'orange' | 'danger';

/** Thresholds from tech.md section 9. */
const THRESHOLDS: ReadonlyArray<readonly [number, UsageTone]> = [
  [90, 'danger'],
  [75, 'orange'],
  [50, 'warn'],
];

/**
 * Rate limit headers are undocumented, so anything can arrive here. Only NaN
 * reads as zero: infinities are ordinary out-of-range numbers and clamp to the
 * end they came from.
 */
export function clampPct(pct: number): number {
  if (Number.isNaN(pct)) return 0;
  return Math.min(100, Math.max(0, pct));
}

/** The step the island announces: every ten percent of the window. tech.md 6.18. */
export const DECADE = 10;

/** Which ten a percent has reached: 30 for 34.2, 0 for 4, null for nothing. */
export function decadeOf(pct: number | null): number | null {
  if (pct === null || !Number.isFinite(pct)) return null;
  return Math.floor(clampPct(pct) / DECADE) * DECADE;
}

/**
 * Whether the window has just stepped into a ten worth announcing.
 *
 * Upward only: the five hour window resets on its own schedule, and falling
 * from 90 to 0 is not news. Never on the first reading either -- there is
 * nothing to compare it against, and an island that flashes on every launch
 * has taught the eye to ignore it by the second one. tech.md 6.18.
 */
export function steppedUp(prev: number | null, next: number | null): boolean {
  const before = decadeOf(prev);
  const after = decadeOf(next);
  if (before === null || after === null) return false;
  return after > before && after >= DECADE;
}

export function usageTone(pct: number): UsageTone {
  const value = clampPct(pct);
  for (const [floor, tone] of THRESHOLDS) {
    if (value >= floor) return tone;
  }
  return 'accent';
}

/**
 * Countdown to the window reset. Copy says when it resets, never how much
 * quota is left: the numbers come off undocumented headers. tech.md 6.4.
 */
export function resetCountdown(resetsAt: number | null, nowSeconds: number): string {
  if (resetsAt === null || !Number.isFinite(resetsAt)) return '';
  const seconds = Math.max(0, Math.round(resetsAt - nowSeconds));
  if (seconds < 60) return 'resets in <1m';

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `resets in ${minutes}m`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    const rest = minutes % 60;
    return rest === 0 ? `resets in ${hours}h` : `resets in ${hours}h ${rest}m`;
  }

  const days = Math.floor(hours / 24);
  const restHours = hours % 24;
  return restHours === 0 ? `resets in ${days}d` : `resets in ${days}d ${restHours}h`;
}
