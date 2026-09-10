/**
 * The window never resizes, so the shape is the only thing that can overflow.
 * tech.md section 10 names these bounds as the property to hold.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
  FALLBACK_NOTCH,
  readNotch,
  REST_PILL,
  REST_SIDE,
  shapeBounds,
  WINDOW,
} from '$lib/logic/shape';
import type { IslandView } from '$lib/types/generated/IslandView';

const views: fc.Arbitrary<IslandView> = fc.oneof(
  fc.constant<IslandView>('Collapsed'),
  fc.constant<IslandView>('Pill'),
  fc.constant<IslandView>('Sessions'),
  fc.string().map((id): IslandView => ({ Session: id })),
);

const notches = fc.record({
  width: fc.double({ noNaN: false }),
  height: fc.double({ noNaN: false }),
});

describe('shape bounds', () => {
  it('never leaves the window, whatever the notch measures', () => {
    fc.assert(
      fc.property(views, notches, fc.boolean(), (view, notch, asking) => {
        const bounds = shapeBounds(view, notch, false, asking);

        expect(Number.isFinite(bounds.width)).toBe(true);
        expect(Number.isFinite(bounds.height)).toBe(true);
        expect(Number.isFinite(bounds.radius)).toBe(true);

        expect(bounds.width).toBeGreaterThanOrEqual(0);
        expect(bounds.height).toBeGreaterThanOrEqual(0);
        expect(bounds.width).toBeLessThanOrEqual(WINDOW.width);
        expect(bounds.height).toBeLessThanOrEqual(WINDOW.height);
      }),
    );
  });

  it('never rounds a corner past the half of the side it sits on', () => {
    fc.assert(
      fc.property(views, notches, fc.boolean(), (view, notch, asking) => {
        const bounds = shapeBounds(view, notch, false, asking);
        expect(bounds.radius).toBeGreaterThanOrEqual(0);
        expect(bounds.radius).toBeLessThanOrEqual(Math.min(bounds.width, bounds.height) / 2);
      }),
    );
  });

  it('is stable: the same view and notch give the same bounds', () => {
    fc.assert(
      fc.property(views, notches, (view, notch) => {
        expect(shapeBounds(view, notch)).toEqual(shapeBounds(view, notch));
      }),
    );
  });

  it('rests around the notch it was measured from, never adrift of it', () => {
    fc.assert(
      fc.property(fc.double({ min: 1, max: 400, noNaN: true }), (width) => {
        const bounds = shapeBounds('Collapsed', { width, height: 32 });
        // Wider than the cutout on purpose: the overhangs are the only pixels
        // a resting island has to draw on. tech.md 6.7.
        expect(bounds.width).toBeCloseTo(width + 2 * REST_SIDE);
        // And exactly as tall as the cutout: a resting island that hangs
        // below it draws a second notch under the real one. tech.md 6.7.
        expect(bounds.height).toBeCloseTo(32);
      }),
    );
  });

  it('grows every open view past the collapsed one', () => {
    const notch = { width: 200, height: 32 };
    const collapsed = shapeBounds('Collapsed', notch);

    for (const view of ['Pill', 'Sessions', { Session: 'abc' }] as IslandView[]) {
      const open = shapeBounds(view, notch);
      expect(open.width).toBeGreaterThan(collapsed.width);
      expect(open.height).toBeGreaterThan(collapsed.height);
      expect(open.radius).toBeGreaterThan(0);
    }
  });
});

describe('reading the notch off the query string', () => {
  it('falls back rather than trusting anything unusable', () => {
    fc.assert(
      fc.property(fc.string(), fc.string(), (a, b) => {
        const notch = readNotch(
          `?notch=${encodeURIComponent(a)}&notch_width=${encodeURIComponent(b)}`,
        );
        expect(Number.isFinite(notch.width)).toBe(true);
        expect(Number.isFinite(notch.height)).toBe(true);
        expect(notch.width).toBeGreaterThan(0);
        expect(notch.height).toBeGreaterThanOrEqual(0);
      }),
    );
  });

  it('reads a real pair back', () => {
    expect(readNotch('?notch=38&notch_width=186')).toEqual({ width: 186, height: 38 });
  });

  it('treats a display with no notch as no notch', () => {
    expect(readNotch('')).toEqual(FALLBACK_NOTCH);
  });
});

describe('a display with no notch', () => {
  it('floats the pill instead of hugging a bezel that is not there', () => {
    const withNotch = shapeBounds('Pill', { width: 185, height: 33 });
    const without = shapeBounds('Pill', { width: 185, height: 0 });

    expect(without.height).toBeLessThan(withNotch.height);
    expect(without.radius).toBeGreaterThan(0);
  });

  it('rests as a small pill rather than a bar across the menu bar', () => {
    const bounds = shapeBounds('Collapsed', { width: 185, height: 0 });
    expect(bounds).toEqual({
      width: REST_PILL.width,
      height: REST_PILL.height,
      radius: REST_PILL.height / 2,
    });
  });

  it('keeps every open view inside the window on any screen', () => {
    fc.assert(
      fc.property(views, fc.double({ min: 0, max: 200, noNaN: true }), (view, height) => {
        const bounds = shapeBounds(view, { width: 185, height });
        expect(bounds.height).toBeLessThanOrEqual(WINDOW.height);
        expect(bounds.width).toBeLessThanOrEqual(WINDOW.width);
      }),
    );
  });
});

/**
 * A question is taller than the dialogue it arrives in: four options with
 * their descriptions run past the bottom edge, and the last of them was cut
 * off by it. tech.md 6.14.
 */
describe('a question standing in the dialogue', () => {
  const notch = { width: 185, height: 34 };

  it('takes the whole window while it stands', () => {
    const asking = shapeBounds({ Session: 'abc' }, notch, false, true);
    const answered = shapeBounds({ Session: 'abc' }, notch, false, false);

    expect(asking.height).toBe(WINDOW.height);
    expect(asking.height).toBeGreaterThan(answered.height);
    // Only the height: a shape that also widened would read as a different
    // panel arriving rather than as this one making room.
    expect(asking.width).toBe(answered.width);
  });

  it('gives it back the moment it is answered', () => {
    expect(shapeBounds({ Session: 'abc' }, notch, false, false)).toEqual(
      shapeBounds({ Session: 'abc' }, notch),
    );
  });

  /// A question is answered in the dialogue and nowhere else. The compact
  /// panel of 6.7 has its own two lines, and the resting island has none.
  it('changes no other view', () => {
    for (const view of ['Collapsed', 'Pill', 'Ask', 'Sessions'] as IslandView[]) {
      expect(shapeBounds(view, notch, false, true)).toEqual(shapeBounds(view, notch, false, false));
    }
  });
});
