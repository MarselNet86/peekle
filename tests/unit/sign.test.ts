/**
 * The sign an empty chat carries, and the rule that there is only one of it.
 * tech.md 6.12 and 9.
 *
 * A dialogue with nothing in it used to be half a window of black over the
 * field, which reads as a screen that did not finish drawing. It carries the
 * product's own mark instead -- the same two strokes the resting mark wears,
 * taken from one place rather than drawn a second time by hand.
 */

import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { SIGN_BOX, SIGN_STROKES, SIGN_WEIGHT } from '$lib/logic/sign';
import RestMark from '$lib/ui/RestMark.svelte';
import Sign from '$lib/ui/Sign.svelte';

/** Every stroke a component drew, in the order it drew them. */
function strokes(container: HTMLElement): string[] {
  return [...container.querySelectorAll('.sign path')].map((path) => path.getAttribute('d') ?? '');
}

describe('the sign', () => {
  it('is the two strokes of the mark, and nothing else', () => {
    const { container } = render(Sign);

    const drawn = [...container.querySelectorAll('path')].map((path) => path.getAttribute('d'));
    expect(drawn).toEqual([...SIGN_STROKES]);
  });

  /// Thickness against length is what the eye recognises the sign by, so both
  /// come from the box rather than from whoever is drawing it.
  it('keeps the proportions of the box at any size', () => {
    const { container } = render(Sign, { props: { size: 88 } });

    const svg = container.querySelector('svg') as SVGElement;
    expect(svg.getAttribute('viewBox')).toBe(`0 0 ${SIGN_BOX.width} ${SIGN_BOX.height}`);
    expect(svg.getAttribute('width')).toBe('88');
    expect(svg.getAttribute('height')).toBe(
      String(Math.round((88 * SIGN_BOX.height) / SIGN_BOX.width)),
    );
    for (const path of container.querySelectorAll('path')) {
      expect(path.getAttribute('stroke-width')).toBe(String(SIGN_WEIGHT));
    }
  });

  /// An empty room says whose window it is and what it is waiting for. The
  /// words belong to whoever places the sign, so the primitive draws none of
  /// its own.
  it('carries a line under the strokes when it is given one', () => {
    const { container } = render(Sign, { props: { caption: "Let's begin" } });

    expect(container.querySelector('.caption')?.textContent).toBe("Let's begin");
  });

  it('stands alone when it is given none', () => {
    const { container } = render(Sign);

    expect(container.querySelector('.caption')).toBeNull();
  });

  /// A reader who hears the island gets one thing said about the empty room,
  /// not the sign and its line one after the other.
  it('says the empty room once, however it is drawn', () => {
    const { container } = render(Sign, { props: { caption: "Let's begin" } });

    const image = container.querySelector('[role="img"]') as HTMLElement;
    expect(container.querySelectorAll('[role="img"]')).toHaveLength(1);
    expect(image.getAttribute('aria-label')).toBe('Nothing said in this chat yet');
    expect(image.querySelector('.caption')?.textContent).toBe("Let's begin");
  });

  /// A standing question wore a pixel question mark from v80.3 to v80.22 and
  /// wears the sign again: the two strokes, in purple. The glyph is the
  /// product's mark and stays in the notch; the colour is what says somebody
  /// is being waited on. tech.md 6.7.
  it('keeps the strokes while a question stands, and only changes colour', () => {
    const { container } = render(RestMark, {
      props: { status: 'waiting', pct: 12, onopen: () => {} },
    });

    expect(strokes(container)).toEqual([...SIGN_STROKES]);
    expect(container.querySelector('.mark')?.getAttribute('data-status')).toBe('waiting');
  });

  /// Every state the mark can be in draws the same geometry now, so none of
  /// them can drift from the others.
  it('draws one geometry in every state', () => {
    for (const status of ['idle', 'working', 'waiting', 'compacting'] as const) {
      const { container, unmount } = render(RestMark, {
        props: { status, pct: 12, onopen: () => {} },
      });

      expect(strokes(container), status).toEqual([...SIGN_STROKES]);
      unmount();
    }
  });

  /// The one rule this slice adds to section 9: the mark and the empty chat
  /// draw the same geometry, because a sign redrawn by hand in a second place
  /// is a sign that drifts from the first.
  it('is the same geometry the resting mark draws', () => {
    const { container: mark } = render(RestMark, {
      props: { status: 'idle', pct: null, onopen: () => {} },
    });
    const { container: sign } = render(Sign);

    expect(strokes(mark)).toEqual(strokes(sign));
    expect(strokes(mark)).toEqual([...SIGN_STROKES]);
  });
});
