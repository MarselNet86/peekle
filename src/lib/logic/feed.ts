/** Feed scrolling. Pure, so the property tests can hammer it. */

/** tech.md 6.8 caps this at 6 no matter what the config says. */
export const MAX_VISIBLE_ROWS = 6;

export interface ScrollState {
  /** Nothing below the fold, so a new message may follow the view down. */
  atBottom: boolean;
  /** The hint shows exactly while there is something below to reach. */
  showHint: boolean;
}

/**
 * Where a scroll container stands.
 *
 * A pixel of slack: browsers report fractional heights on scaled displays, and
 * an exact comparison leaves the hint lit at the very bottom of every feed.
 */
export function scrollState(
  scrollTop: number,
  clientHeight: number,
  scrollHeight: number,
): ScrollState {
  const sane = (value: number) => (Number.isFinite(value) ? Math.max(value, 0) : 0);
  const top = sane(scrollTop);
  const visible = sane(clientHeight);
  const total = sane(scrollHeight);

  const below = total - top - visible;
  const atBottom = below <= 1;
  return { atBottom, showHint: !atBottom };
}
