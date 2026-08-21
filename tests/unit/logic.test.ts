/**
 * Property based tests for the pure frontend logic. tech.md section 10 names
 * the usage window math and the feed window as the two that need them.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { clampPct, resetCountdown, usageTone } from '$lib/logic/usage';
import { scrollState } from '$lib/logic/feed';

describe('usage math', () => {
  it('lands in 0..100 for any number the headers can produce', () => {
    fc.assert(
      fc.property(fc.double({ noNaN: false }), (pct) => {
        const value = clampPct(pct);
        expect(value).toBeGreaterThanOrEqual(0);
        expect(value).toBeLessThanOrEqual(100);
        expect(Number.isNaN(value)).toBe(false);
      }),
    );
  });

  it('is idempotent', () => {
    fc.assert(
      fc.property(fc.double({ noNaN: false }), (pct) => {
        expect(clampPct(clampPct(pct))).toBe(clampPct(pct));
      }),
    );
  });

  it('is monotone', () => {
    fc.assert(
      fc.property(fc.double({ noNaN: true }), fc.double({ noNaN: true }), (a, b) => {
        fc.pre(a <= b);
        expect(clampPct(a)).toBeLessThanOrEqual(clampPct(b));
      }),
    );
  });

  it('never leaves a percentage without a tone', () => {
    fc.assert(
      fc.property(fc.double({ noNaN: false }), (pct) => {
        expect(['accent', 'warn', 'orange', 'danger']).toContain(usageTone(pct));
      }),
    );
  });

  it('walks the thresholds in order', () => {
    expect(usageTone(0)).toBe('accent');
    expect(usageTone(49.9)).toBe('accent');
    expect(usageTone(50)).toBe('warn');
    expect(usageTone(74.9)).toBe('warn');
    expect(usageTone(75)).toBe('orange');
    expect(usageTone(89.9)).toBe('orange');
    expect(usageTone(90)).toBe('danger');
    expect(usageTone(100)).toBe('danger');
  });

  it('says when a window resets and never how much is left', () => {
    const now = 1_700_000_000;
    expect(resetCountdown(now + 30, now)).toBe('resets in <1m');
    expect(resetCountdown(now + 600, now)).toBe('resets in 10m');
    expect(resetCountdown(now + 5400, now)).toBe('resets in 1h 30m');
    expect(resetCountdown(now + 7200, now)).toBe('resets in 2h');
    expect(resetCountdown(now + 300_000, now)).toBe('resets in 3d 11h');
    expect(resetCountdown(null, now)).toBe('');
    // A reset already in the past reads as imminent, not negative.
    expect(resetCountdown(now - 500, now)).toBe('resets in <1m');
  });
});

describe('feed scrolling', () => {
  it('hints exactly while something is below the fold', () => {
    fc.assert(
      fc.property(
        fc.double({ min: 0, max: 4000, noNaN: true }),
        fc.double({ min: 1, max: 800, noNaN: true }),
        fc.double({ min: 0, max: 8000, noNaN: true }),
        (top, clientHeight, extra) => {
          const scrollHeight = clientHeight + extra;
          const state = scrollState(top, clientHeight, scrollHeight);

          // The two are the same fact stated twice, so they can never disagree.
          expect(state.showHint).toBe(!state.atBottom);
          const below = scrollHeight - top - clientHeight;
          expect(state.atBottom).toBe(below <= 1);
        },
      ),
    );
  });

  it('reads a feed shorter than its box as being at the bottom', () => {
    expect(scrollState(0, 400, 400)).toEqual({ atBottom: true, showHint: false });
    expect(scrollState(0, 400, 120)).toEqual({ atBottom: true, showHint: false });
  });

  it('reads a feed scrolled to the very end as being at the bottom', () => {
    expect(scrollState(600, 400, 1000).atBottom).toBe(true);
    // Half a pixel of rounding on a scaled display is still the bottom.
    expect(scrollState(599.5, 400, 1000).atBottom).toBe(true);
    expect(scrollState(560, 400, 1000).atBottom).toBe(false);
  });

  it('survives the numbers a detached element reports', () => {
    expect(scrollState(NaN, NaN, NaN)).toEqual({ atBottom: true, showHint: false });
    expect(scrollState(-10, 0, 0)).toEqual({ atBottom: true, showHint: false });
  });
});
