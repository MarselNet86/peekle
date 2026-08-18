/**
 * The window never resizes, so the shape is the only thing that can overflow.
 * tech.md section 10 names these bounds as the property to hold.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { FALLBACK_NOTCH, readNotch, shapeBounds, WINDOW } from '$lib/logic/shape';
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
      fc.property(views, notches, (view, notch) => {
        const bounds = shapeBounds(view, notch);

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
      fc.property(views, notches, (view, notch) => {
        const bounds = shapeBounds(view, notch);
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

  it('collapses to the notch itself, never wider', () => {
    fc.assert(
      fc.property(fc.double({ min: 1, max: 400, noNaN: true }), (width) => {
        const bounds = shapeBounds('Collapsed', { width, height: 32 });
        expect(bounds.width).toBeCloseTo(width);
        expect(bounds.radius).toBe(0);
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
