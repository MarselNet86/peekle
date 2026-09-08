/**
 * v58 acceptance. The resting island announces one thing about usage -- the
 * five hour window stepping into a new ten -- and it announces it the way the
 * system does: the shape makes room, the number steps out beside the ring,
 * and both go back. tech.md 6.18.
 */

import { render, screen } from '@testing-library/svelte';
import fc from 'fast-check';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { DECADE, decadeOf, steppedUp } from '$lib/logic/usage';
import { REST_BADGE, REST_SIDE, WINDOW, shapeBounds } from '$lib/logic/shape';
import RestMark from '$lib/ui/RestMark.svelte';
import type { IslandView } from '$lib/types/generated/IslandView';

const notch = { width: 200, height: 32 };

const badgeCalls = vi.hoisted(() => ({ on: true, set: vi.fn() }));

vi.mock('$lib/bridge', () => ({
  commands: {
    usageBadge: () => Promise.resolve(badgeCalls.on),
    setUsageBadge: (on: boolean) => {
      badgeCalls.set(on);
      return Promise.resolve();
    },
  },
}));

describe('steppedUp', () => {
  it('announces a ten the window has just crossed', () => {
    expect(steppedUp(9, 10)).toBe(true);
    expect(steppedUp(29.4, 31)).toBe(true);
  });

  it('says nothing inside a ten', () => {
    expect(steppedUp(31, 34)).toBe(false);
    expect(steppedUp(70, 70)).toBe(false);
  });

  it('says nothing when the window resets', () => {
    // The five hour window empties on its own schedule. That is not news.
    expect(steppedUp(94, 0)).toBe(false);
    expect(steppedUp(50, 41)).toBe(false);
  });

  it('says nothing on the first reading', () => {
    // Nothing to compare against, and an island that flashes on every launch
    // has taught the eye to ignore it by the second one.
    expect(steppedUp(null, 40)).toBe(false);
  });

  it('never announces a step down, whatever the numbers', () => {
    fc.assert(
      fc.property(
        fc.double({ min: -1000, max: 1000, noNaN: false }),
        fc.double({ min: -1000, max: 1000, noNaN: false }),
        (prev, next) => {
          if (steppedUp(prev, next)) {
            const after = decadeOf(next);
            const before = decadeOf(prev);
            expect(after).not.toBeNull();
            expect(before).not.toBeNull();
            expect(after as number).toBeGreaterThan(before as number);
            expect(after as number).toBeGreaterThanOrEqual(DECADE);
          }
        },
      ),
    );
  });
});

describe('shapeBounds with the badge', () => {
  it('makes room on both sides, so the notch stays in the middle', () => {
    const rest = shapeBounds('Collapsed' as IslandView, notch);
    const wide = shapeBounds('Collapsed' as IslandView, notch, true);

    expect(rest.width).toBe(notch.width + 2 * REST_SIDE);
    expect(wide.width).toBe(rest.width + 2 * REST_BADGE);
    expect(wide.height).toBe(rest.height);
  });

  it('grows the pill on a display with no notch to hang from', () => {
    const flat = { width: 0, height: 0 };
    const rest = shapeBounds('Collapsed' as IslandView, flat);
    const wide = shapeBounds('Collapsed' as IslandView, flat, true);

    expect(wide.width).toBe(rest.width + 2 * REST_BADGE);
  });

  it('leaves an open island alone: its header already shows the number', () => {
    for (const view of ['Pill', 'Sessions', { Session: 'x' }] as IslandView[]) {
      expect(shapeBounds(view, notch, true)).toEqual(shapeBounds(view, notch));
    }
  });

  it('stays inside the window at any notch', () => {
    fc.assert(
      fc.property(
        fc.double({ min: 0, max: 2000, noNaN: true }),
        fc.double({ min: 0, max: 200, noNaN: true }),
        (width, height) => {
          const bounds = shapeBounds('Collapsed' as IslandView, { width, height }, true);
          expect(bounds.width).toBeLessThanOrEqual(WINDOW.width);
          expect(bounds.height).toBeLessThanOrEqual(WINDOW.height);
        },
      ),
    );
  });
});

describe('createBadge', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    badgeCalls.on = true;
    badgeCalls.set.mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.resetModules();
  });

  async function badge() {
    const { createBadge } = await import('$lib/features/usage/badge.svelte');
    const made = createBadge();
    await made.start();
    return made;
  }

  it('shows the percent when the window steps into a ten', async () => {
    const it = await badge();
    it.track(9);
    expect(it.value).toBeNull();

    it.track(10.4);
    expect(it.value).toBe(10);
    expect(it.wide).toBe(true);
  });

  it('takes the number away first and the shape after it', async () => {
    const { BADGE_HOLD_MS, BADGE_FADE_MS } = await import('$lib/features/usage/badge.svelte');
    const it = await badge();
    it.track(9);
    it.track(20);

    vi.advanceTimersByTime(BADGE_HOLD_MS);
    // A shape that collapses under its own content reads as an interruption.
    expect(it.value).toBeNull();
    expect(it.wide).toBe(true);

    vi.advanceTimersByTime(BADGE_FADE_MS);
    expect(it.wide).toBe(false);
  });

  it('stays shut while the switch is off', async () => {
    badgeCalls.on = false;
    const it = await badge();
    it.track(9);
    it.track(40);

    expect(it.value).toBeNull();
    expect(it.wide).toBe(false);
  });

  it('holds what the switch turned on until the island rests', async () => {
    // The switch is inside an open island; the badge lives on a resting one.
    badgeCalls.on = false;
    const it = await badge();
    await it.set(true, 62);

    expect(badgeCalls.set).toHaveBeenCalledWith(true);
    expect(it.on).toBe(true);
    expect(it.value).toBeNull();

    it.rest();
    expect(it.value).toBe(62);
    expect(it.wide).toBe(true);
  });

  it('holds nothing when there was nothing to promise', async () => {
    const it = await badge();
    it.rest();
    it.rest();

    expect(it.value).toBeNull();
    expect(it.wide).toBe(false);
  });

  it('drops a promise the switch took back', async () => {
    badgeCalls.on = false;
    const it = await badge();
    await it.set(true, 62);
    await it.set(false, 62);
    it.rest();

    expect(it.value).toBeNull();
  });

  it('clears the island the moment the switch goes off', async () => {
    const it = await badge();
    it.track(9);
    it.track(30);
    expect(it.value).toBe(30);

    await it.set(false, 30);
    expect(it.value).toBeNull();
    expect(it.wide).toBe(false);
  });
});

describe('RestMark', () => {
  it('writes the percent beside the ring', () => {
    render(RestMark, { props: { status: 'idle', pct: 70, badge: 70, onopen: () => {} } });

    expect(screen.getByText('70%')).toBeInTheDocument();
  });

  it('keeps the number in the markup on the way out, so it can leave', () => {
    const { container, rerender } = render(RestMark, {
      props: { status: 'idle', pct: 70, badge: 70, onopen: () => {} },
    });
    expect(container.querySelector('.pct')).toHaveClass('standing');

    rerender({ status: 'idle', pct: 70, badge: null, onopen: () => {} });
    const pct = container.querySelector('.pct');
    expect(pct).not.toBeNull();
    expect(pct).not.toHaveClass('standing');
    expect(pct?.textContent).toBe('70%');
  });

  it('carries the same colour the ring carries', () => {
    const { container } = render(RestMark, {
      props: { status: 'idle', pct: 92, badge: 92, onopen: () => {} },
    });

    expect(container.querySelector('.pct')?.getAttribute('data-tone')).toBe('danger');
  });
});
