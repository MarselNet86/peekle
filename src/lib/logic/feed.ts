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

/** Where a scroller should stand after a view change or a new row. */
export type ScrollAim = 'top' | 'bottom' | 'stay';

/**
 * Where the island's one scroller lands.
 *
 * The list and the feed share it, and they want opposite ends. A dialogue
 * opens on its last message, because that is what it was opened for, and it
 * follows new rows down only while the reader was already at the bottom. A
 * list opens on its first row, because it is sorted freshest first and its
 * end is the oldest thing it has. Neither is followed anywhere while the
 * reader has scrolled off on their own. tech.md 6.12.
 */
export function scrollAim(kind: 'list' | 'feed', switched: boolean, atBottom: boolean): ScrollAim {
  if (kind === 'list') return switched ? 'top' : 'stay';
  if (switched) return 'bottom';
  return atBottom ? 'bottom' : 'stay';
}
