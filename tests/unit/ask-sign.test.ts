/**
 * The question mark the resting island wears while a question stands.
 * tech.md 6.7 and 9.
 *
 * Acceptance, from the change: the two strokes are the product's sign, and a
 * question is not the product. While the agent waits on the person the mark
 * draws a question mark instead -- pixels, purple, breathing -- and every
 * other state keeps the sign it always wore.
 */

import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ASK_PIXEL, ASK_PIXELS, ASK_ROWS } from '$lib/logic/ask-sign';
import type { RestStatus } from '$lib/logic/rest';
import { SIGN_BOX } from '$lib/logic/sign';
import RestMark from '$lib/ui/RestMark.svelte';

const mark = (status: RestStatus) =>
  render(RestMark, { props: { status, pct: 12, onopen: () => {} } });

/** Every pixel a component drew, as `x,y` in the order it drew them. */
function pixels(container: HTMLElement): string[] {
  return [...container.querySelectorAll('.ask rect')].map(
    (rect) => `${rect.getAttribute('x')},${rect.getAttribute('y')}`,
  );
}

describe('the question mark', () => {
  it('is drawn from the geometry, never by the component', () => {
    const { container } = mark('waiting');

    expect(pixels(container)).toEqual(ASK_PIXELS.map(({ x, y }) => `${x},${y}`));
    expect(pixels(container)).toHaveLength(ASK_ROWS.join('').split('#').length - 1);
  });

  /// A pixel is a pixel: same side on every one, on a whole-unit grid, so no
  /// edge of the glyph lands on half a device pixel and blurs.
  it('lays whole pixels on one grid', () => {
    const { container } = mark('waiting');

    const rects = [...container.querySelectorAll('.ask rect')];
    expect(rects.length).toBeGreaterThan(0);
    for (const rect of rects) {
      expect(rect.getAttribute('width')).toBe(String(ASK_PIXEL));
      expect(rect.getAttribute('height')).toBe(String(ASK_PIXEL));
      const x = Number(rect.getAttribute('x'));
      const y = Number(rect.getAttribute('y'));
      expect(Number.isInteger(x)).toBe(true);
      expect(Number.isInteger(y)).toBe(true);
      expect(x).toBeGreaterThanOrEqual(0);
      expect(y).toBeGreaterThanOrEqual(0);
      expect(x + ASK_PIXEL).toBeLessThanOrEqual(SIGN_BOX.width);
      expect(y + ASK_PIXEL).toBeLessThanOrEqual(SIGN_BOX.height);
    }
  });

  /// The glyph swaps into the box the sign leaves, so the mark it replaces
  /// stood in the same place: an island whose glyph jumps sideways on a
  /// question reads as two marks, not one changing its mind.
  it('stands in the middle of the box the sign is drawn in', () => {
    const xs = ASK_PIXELS.map(({ x }) => x);
    const ys = ASK_PIXELS.map(({ y }) => y);
    const left = Math.min(...xs);
    const right = Math.max(...xs) + ASK_PIXEL;
    const top = Math.min(...ys);
    const bottom = Math.max(...ys) + ASK_PIXEL;

    expect(left).toBe(SIGN_BOX.width - right);
    expect(top).toBe(SIGN_BOX.height - bottom);
  });

  /// Rule 10 of the mark's own reading: the sign means Peekle, and swapping
  /// the whole glyph is what keeps it from meaning anything else.
  it('takes the place of the sign rather than standing beside it', () => {
    const { container } = mark('waiting');

    expect(container.querySelectorAll('.stroke')).toHaveLength(0);
    expect(container.querySelector('.sign')).toBeNull();
    expect(container.querySelector('.spin')).toBeNull();
  });

  it('is drawn in no other state', () => {
    for (const status of ['idle', 'working', 'compacting'] as const) {
      const { container, unmount } = mark(status);

      expect(container.querySelector('.ask')).toBeNull();
      expect(container.querySelectorAll('.stroke')).toHaveLength(2);
      unmount();
    }
  });

  /// The colour and the breath are the waiting mark's own, and they hang off
  /// `data-status` the way every other state's do: the glyph changed, the hook
  /// the stylesheet reaches it by did not. What the purple looks like is a
  /// question for the shot, not for jsdom, which loads no stylesheet at all.
  it('keeps the hook its colour and its breath hang from', () => {
    const { container } = mark('waiting');

    expect(container.querySelector('.mark')?.getAttribute('data-status')).toBe('waiting');
    expect(container.querySelector('.glyph .ask')).toBeInstanceOf(SVGElement);
  });
});
