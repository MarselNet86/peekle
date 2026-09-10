/**
 * The question mark the resting island wears while the agent waits on a
 * person, as geometry. tech.md 6.7 and 9.
 *
 * Pixels rather than strokes, and on purpose: the sign `//` is the product,
 * and the product is not what the island is saying here. A question is, and it
 * has to be told apart from `//` at a glance, from across a screen, in the
 * eight by twelve pixels a notch overhang gives it. A glyph built out of whole
 * pixels reads at that size where a drawn curve turns to mush, and it is the
 * shape a terminal would have drawn -- which is where the question came from.
 *
 * It lives here rather than inside the component for the same reason the sign
 * does: geometry written once is geometry that cannot drift, and a bitmap is
 * worth checking by test rather than by eye.
 */

import { SIGN_BOX } from './sign';

/** The side of one pixel, in the units of `SIGN_BOX`. Whole units, because a
 * pixel landing on half a device pixel is a blurred pixel, and a blurred pixel
 * is the one thing this glyph cannot afford. */
export const ASK_PIXEL = 2;

/**
 * The glyph as a bitmap, top row first: `#` is a pixel, anything else is the
 * gap around one.
 *
 * Five wide, and that is the whole of why it reads straight. On an even width
 * the stem cannot sit in the middle of the bowl -- it lands half a pixel off
 * and the glyph leans, which is exactly how the four wide one looked. Odd
 * width puts the stem and the dot on the centre column, and the bowl falls
 * into it one step at a time: the shape a small bitmap font draws.
 */
export const ASK_ROWS = ['.###.', '#...#', '...#.', '..#..', '.....', '..#..'] as const;

/** How long one pixel waits behind the one before it, assembling and coming
 * apart again. Slow enough to be seen laying itself out from across a screen,
 * quick enough that the glyph stands whole for most of the cycle. */
export const ASK_STEP_MS = 110;

/** The whole cycle: it lays itself out, stands, comes apart in the same order,
 * and the box is empty for a beat before it starts again. */
export const ASK_CYCLE_MS = 2600;

/** How much of the cycle one pixel stays on for, as the percentage the
 * stylesheet writes into its keyframes. Everything below is checked against
 * it: the glyph has to stand whole for a moment, and come fully apart before
 * the next round starts. */
export const ASK_ON_PCT = 58;

/** One drawn pixel, by the corner it starts at, in box units. */
export type AskPixel = { x: number; y: number };

/** The bitmap laid into the box the sign is drawn in, so the two glyphs swap
 * in place and neither has to know about the other. */
function laid(rows: readonly string[]): AskPixel[] {
  const wide = Math.max(...rows.map((row) => row.length));
  const left = (SIGN_BOX.width - wide * ASK_PIXEL) / 2;
  const top = (SIGN_BOX.height - rows.length * ASK_PIXEL) / 2;

  return rows.flatMap((row, y) =>
    [...row].flatMap((cell, x) =>
      cell === '#' ? [{ x: left + x * ASK_PIXEL, y: top + y * ASK_PIXEL }] : [],
    ),
  );
}

/**
 * Every pixel of the glyph, left to right and top to bottom.
 *
 * The order is the order it assembles in and the order it comes apart in --
 * the bowl first, then the stem, then the dot -- so the index of a pixel here
 * is the delay it is drawn with. tech.md 6.7.
 */
export const ASK_PIXELS: readonly AskPixel[] = laid(ASK_ROWS);
