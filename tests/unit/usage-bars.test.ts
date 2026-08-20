/**
 * S7. Every UsageUnavailable variant has to render, because unavailable is a
 * normal state rather than an error: the bars go to dashes with the reason
 * spelled out and the island keeps working. tech.md 6.4 and R-3.
 */

import { render, screen } from '@testing-library/svelte';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { bars, reasonText, WINDOW_LABELS, needsGrant } from '$lib/features/usage/usage.svelte';
import UsageBar from '$lib/ui/UsageBar.svelte';
import type { UsageSnapshot } from '$lib/types/generated/UsageSnapshot';
import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

const live: UsageSnapshot = {
  windows: [
    { window: 'FiveHour', used_pct: 17, resets_at: 1787140200 },
    { window: 'SevenDay', used_pct: 48, resets_at: 1787151600 },
  ],
  source: 'Account',
  reason: null,
  fetched_at: 0,
};

const REASONS: UsageUnavailable[] = [
  'Disabled',
  'NotGranted',
  'Denied',
  'NotLoggedIn',
  'Network',
  'Unsupported',
];

describe('the two bars', () => {
  it('are always both there, in the order of the contract', () => {
    const drawn = bars(live);
    expect(drawn.map((b) => b.label)).toEqual([WINDOW_LABELS.FiveHour, WINDOW_LABELS.SevenDay]);
    expect(drawn[0].pct).toBe(17);
    expect(drawn[1].pct).toBe(48);
  });

  it('go to dashes rather than zero when there is no snapshot', () => {
    for (const drawn of bars(null)) {
      expect(drawn.pct).toBeNull();
      expect(drawn.resetsAt).toBeNull();
    }
  });

  it('go to dashes for a window the response did not carry', () => {
    const partial: UsageSnapshot = { ...live, windows: [live.windows[0]] };
    const drawn = bars(partial);

    expect(drawn[0].pct).toBe(17);
    expect(drawn[1].pct).toBeNull();
  });
});

describe('the reason', () => {
  it('says something specific for every variant', () => {
    const seen = new Set<string>();

    for (const reason of REASONS) {
      const text = reasonText({ ...live, windows: [], source: 'Unavailable', reason });
      expect(text.length).toBeGreaterThan(0);
      expect(text).not.toBe('unavailable');
      seen.add(text);
    }
    // Distinct wording, or the user cannot tell the states apart.
    expect(seen.size).toBe(REASONS.length);
  });

  it('never mentions a limit or a quota', () => {
    for (const reason of REASONS) {
      const text = reasonText({ ...live, windows: [], source: 'Unavailable', reason });
      expect(text.toLowerCase()).not.toContain('limit');
      expect(text.toLowerCase()).not.toContain('quota');
    }
  });
});

describe('UsageBar', () => {
  it('draws dashes and the reason when the number is unknown', () => {
    render(UsageBar, { props: { label: '5h', pct: null, reason: 'needs Keychain access' } });

    expect(screen.getByText('––')).toBeInTheDocument();
    expect(screen.getByText('needs Keychain access')).toBeInTheDocument();
  });

  it('renders any percentage the headers could produce without breaking', () => {
    fc.assert(
      fc.property(fc.double({ noNaN: false }), (pct) => {
        const { container, unmount } = render(UsageBar, { props: { label: '5h', pct } });

        const fill = container.querySelector('.fill') as HTMLElement | null;
        if (fill) {
          const width = parseFloat(fill.style.width);
          expect(width).toBeGreaterThanOrEqual(0);
          expect(width).toBeLessThanOrEqual(100);
        }
        unmount();
      }),
    );
  });
});

describe('the grant control', () => {
  const snapshot = (reason: UsageUnavailable | null): UsageSnapshot => ({
    windows: [],
    source: 'Unavailable',
    reason,
    fetched_at: 0,
  });

  /// The reasons the user can fix from where they are standing.
  it('offers itself when the Keychain is the thing in the way', () => {
    expect(needsGrant(snapshot('NotGranted'))).toBe(true);
    expect(needsGrant(snapshot('Denied'))).toBe(true);
  });

  /// Offering a Keychain dialog for a switch in the config, or for a shape the
  /// API stopped sending, sends the user to press something that cannot help.
  it('stays out of the way when the Keychain is not the problem', () => {
    expect(needsGrant(snapshot('Disabled'))).toBe(false);
    expect(needsGrant(snapshot('Unsupported'))).toBe(false);
    expect(needsGrant(snapshot('NotLoggedIn'))).toBe(false);
    expect(needsGrant(snapshot('Network'))).toBe(false);
    expect(needsGrant(snapshot(null))).toBe(false);
    expect(needsGrant(null)).toBe(false);
  });
});
