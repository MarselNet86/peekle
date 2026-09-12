/**
 * Island geometry. Pure, so the property tests can hammer it.
 *
 * The window never resizes (tech.md 6.7), so every shape has to fit inside the
 * bounds below. Anything wider or taller would be clipped by the webview with
 * no error, which is exactly the failure that is hard to see in a screenshot.
 */

import type { IslandView } from '$lib/types/generated/IslandView';

export const WINDOW = { width: 720, height: 560 } as const;

/** Stand-in for a display with no notch to measure. */
export const FALLBACK_NOTCH = { width: 200, height: 0 } as const;

/**
 * How far it overhangs the notch on each side.
 *
 * A notch is the absence of pixels, so anything drawn across its width cannot
 * be seen at all. The overhangs are the real pixels beside the cutout, and
 * they are what carries the mark. tech.md 6.7.
 */
export const REST_SIDE = 38;

/**
 * The mark on a display with no bezel to hang from: the capsule of the iPhone
 * island, floating below the edge. Stretching it to `FALLBACK_NOTCH` would lay
 * a black bar across the middle of the menu bar; the 78 by 20 pill that stood
 * here before v82 was a crumb at the edge of a 1920 pixel screen, neither
 * recognisable nor pressable. tech.md 6.7.
 *
 * 132 by 36 was the first live size, and the first live Windows run said it
 * back: a capsule that wide sits across the tab strip of whatever is
 * maximised under it and reads as a black bar somebody left on the screen.
 * 104 by 28 still carries the sign and the ring with room between them, still
 * gives the pointer a target it cannot miss, and stops pretending to be a
 * notch on a screen that has none. tech.md 6.7 and 6.27.
 */
export const REST_FLOAT = { width: 104, height: 28 } as const;

/**
 * How far every shape stands off the top edge on a display with no notch.
 * Zero with one: the black there continues the cutout. tech.md 6.7.
 */
export const FLOAT_TOP = 6;

/**
 * The band the top row of a list or a dialogue stands in, on a display with no
 * notch.
 *
 * Under a cutout that band is the cutout: the row is lifted into it and costs
 * the content below nothing. Without one there is nothing to lift into, and a
 * row flush with the edge of a shape rounded on every corner has its ends cut
 * off by the corners -- the gear and the bug of the list sat in the curve, and
 * so did the usage dials of a dialogue. So the row gets a band of its own
 * here, and the shape grows by exactly it. tech.md 6.7.
 */
export const FLOAT_HEAD = 12;

/**
 * How much wider a resting shape gets on each side while the usage badge
 * stands.
 *
 * Symmetric, and it has to be: the notch is a hole in the middle of the
 * screen, and the black around it is only convincing while the shape stays
 * centred on it. So the left edge gains empty fill and the right edge gains
 * the room the number needs. tech.md 6.18.
 */
export const REST_BADGE = 28;

export interface Notch {
  width: number;
  height: number;
}

export interface ShapeBounds {
  width: number;
  height: number;
  /** Bottom corners under a notch, where the top is cut by the screen edge;
   * every corner on a display without one. tech.md 6.10 and 6.7. */
  radius: number;
}

/** The gap between the shape and the top edge: none under a notch, where the
 * black continues the cutout, and `FLOAT_TOP` without one. tech.md 6.7. */
export function shapeInset(notch: Notch): number {
  return sane(notch.height, FALLBACK_NOTCH.height) > 0 ? 0 : FLOAT_TOP;
}

/** Rust hands these over in the query string and either can be absent. */
export function readNotch(search: string): Notch {
  const params = new URLSearchParams(search);
  return {
    width: finite(params.get('notch_width'), FALLBACK_NOTCH.width),
    height: finite(params.get('notch'), FALLBACK_NOTCH.height),
  };
}

function finite(raw: string | null, fallback: number): number {
  const value = Number(raw);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function sane(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

/**
 * Target bounds of the black shape for a view.
 *
 * Collapsed returns the notch plus the resting drop rather than zero: the
 * spring has to grow out of the bezel, and a shape that starts at nothing
 * reads as a window appearing rather than as the notch opening.
 *
 * `deep` is a pill with a second line under its first: the end of a turn says
 * who finished and what they said, and one line cannot hold both. tech.md 6.2.
 *
 * `asking` is a question standing in the dialogue. Four options with their
 * descriptions are taller than the 420 a dialogue stands at, and the last of
 * them was cut off by the bottom edge -- an option nobody can see is an option
 * that is not there. So the shape takes the whole window for as long as the
 * question stands and gives it back the moment it is answered. There is
 * nowhere further to grow: the window itself never resizes (6.7), and what
 * still does not fit scrolls inside the panel. tech.md 6.14.
 */
export function shapeBounds(
  view: IslandView,
  notch: Notch,
  badge = false,
  asking = false,
  deep = false,
): ShapeBounds {
  const width = sane(notch.width, FALLBACK_NOTCH.width);
  const height = sane(notch.height, FALLBACK_NOTCH.height);

  // Only a resting island carries the badge. An open one shows the same number
  // on the dials in its header, and a third copy of it is not news. tech.md 6.18.
  const grown = badge && view === 'Collapsed' ? 2 * REST_BADGE : 0;

  // Collapsed keeps exactly the height of the cutout and spends every pixel of
  // its growth on the sides. A strip hanging below the notch reads as a second
  // notch painted under the real one, which is the one giveaway the island
  // exists to avoid. tech.md 6.7.
  // Without a notch the shape floats, and floats as the iPhone island does:
  // every corner round, the short views round at the ends, the tall ones on
  // a wide radius. A shape that grows out of nothing needs no straight top.
  // tech.md 6.7.
  const floating = height === 0;

  // The band the top row stands in. The same pixels under a notch, where they
  // are the cutout; without one they are a plain inset, and only the two views
  // carrying such a row take them -- a pill and a permission panel have no top
  // row to keep out of the corners. tech.md 6.7.
  const band = floating ? FLOAT_HEAD : height;
  const bounds =
    view === 'Collapsed'
      ? floating
        ? {
            width: REST_FLOAT.width + grown,
            height: REST_FLOAT.height,
            radius: REST_FLOAT.height / 2,
          }
        : { width: width + 2 * REST_SIDE + grown, height, radius: 12 }
      : view === 'Pill'
        ? // A pill carrying a second line is the height of the panel that
          // carries two: one shape for two lines, whatever is on them.
          // tech.md 6.2 and 6.7.
          {
            width: 420,
            height: height + (deep ? 62 : 44),
            radius: floating ? (deep ? 31 : 22) : 20,
          }
        : // Two lines and two buttons, and nothing else: the answer to a
          // permission needs no feed. tech.md 6.7.
          view === 'Ask'
          ? { width: 460, height: height + 62, radius: floating ? 31 : 22 }
          : view === 'Sessions'
            ? { width: 460, height: band + 380, radius: floating ? 28 : 24 }
            : {
                width: 560,
                height: asking ? WINDOW.height : band + 420,
                radius: floating ? 28 : 24,
              };

  return clamp(bounds);
}

function clamp(bounds: ShapeBounds): ShapeBounds {
  const width = Math.min(Math.max(bounds.width, 0), WINDOW.width);
  const height = Math.min(Math.max(bounds.height, 0), WINDOW.height);
  return {
    width,
    height,
    radius: Math.min(Math.max(bounds.radius, 0), width / 2, height / 2),
  };
}
