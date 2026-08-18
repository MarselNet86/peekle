/**
 * Property based tests for the pure frontend logic. tech.md section 10 names
 * the usage window math and the feed window as the two that need them.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { clampPct, resetCountdown, usageTone } from '$lib/logic/usage';
import { feedWindow, MAX_VISIBLE_ROWS } from '$lib/logic/feed';
import type { TaskItem } from '$lib/types/generated/TaskItem';

const task = (i: number): TaskItem => ({
  id: String(i),
  title: `task ${i}`,
  label: 'Code',
  status: 'Pending',
  session_id: 's',
  updated_at: i,
});

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

describe('feed window', () => {
  it('never renders more than six rows and hints exactly when it clips', () => {
    fc.assert(
      fc.property(fc.nat({ max: 200 }), (count) => {
        const tasks = Array.from({ length: count }, (_, i) => task(i));
        const view = feedWindow(tasks);

        expect(view.visible.length).toBeLessThanOrEqual(MAX_VISIBLE_ROWS);
        expect(view.visible.length).toBe(Math.min(count, MAX_VISIBLE_ROWS));
        expect(view.showScrollHint).toBe(count > MAX_VISIBLE_ROWS);
        expect(view.showList).toBe(count > 0);
      }),
    );
  });

  it('clamps a configured limit above six back down to six', () => {
    fc.assert(
      fc.property(fc.integer({ min: -20, max: 400 }), (limit) => {
        const tasks = Array.from({ length: 40 }, (_, i) => task(i));
        expect(feedWindow(tasks, limit).visible.length).toBeLessThanOrEqual(MAX_VISIBLE_ROWS);
      }),
    );
  });

  it('hides the list when there is nothing to show', () => {
    expect(feedWindow([]).showList).toBe(false);
    expect(feedWindow([]).showScrollHint).toBe(false);
  });
});
