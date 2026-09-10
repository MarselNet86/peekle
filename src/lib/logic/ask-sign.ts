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
 * gap around one. The bowl, its right side falling to the stem, then the gap
 * and the dot -- the same seven pixels a small bitmap font spends on `?`.
 */
export const ASK_ROWS = ['.##.', '#..#', '...#', '..#.', '....', '..#.'] as const;

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

/** Every pixel of the glyph, left to right and top to bottom. */
export const ASK_PIXELS: readonly AskPixel[] = laid(ASK_ROWS);
