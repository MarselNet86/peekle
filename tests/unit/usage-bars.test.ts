/**
 * S7. Every UsageUnavailable variant has to render, because unavailable is a
 * normal state rather than an error: the bars go to dashes with the reason
 * spelled out and the island keeps working. tech.md 6.4 and R-3.
 */

import { render, screen } from '@testing-library/svelte';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
  bars,
  connectLabel,
  gateSessions,
  reasonText,
  WINDOW_LABELS,
} from '$lib/features/usage/usage.svelte';
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
  keychain_granted: true,
};

const REASONS: UsageUnavailable[] = [
  'Disabled',
  'NotGranted',
  'Denied',
  'NotLoggedIn',
  'Offline',
  'Network',
  'RateLimited',
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
  const snapshot = (reason: UsageUnavailable | null, granted = false): UsageSnapshot => ({
    windows: [],
    source: 'Unavailable',
    reason,
    fetched_at: 0,
    keychain_granted: granted,
  });

  /// A first run and a dropped session are one press apart from working, and
  /// the copy is the only thing that differs.
  it('says connect on a first run and reconnect on a session that dropped', () => {
    expect(connectLabel(snapshot('NotGranted'))).toBe('Connect');
    expect(connectLabel(snapshot('Denied'))).toBe('Connect');
    expect(connectLabel(snapshot('Offline'))).toBe('Reconnect');
    expect(connectLabel(snapshot('Network'))).toBe('Reconnect');
  });

  /// A missing or expired credential is not fixed by re-reading the Keychain
  /// either, so it left this control for one that runs Claude Code's own
  /// login. tech.md 6.16, and `signin.test.ts` holds that side.
  it('leaves a missing credential to the sign-in', () => {
    expect(connectLabel(snapshot('NotLoggedIn'))).toBeNull();
  });

  /// A switch in the config and a body that changed shape are not fixed by a
  /// Keychain dialog.
  it('stays out of the way when pressing it could not help', () => {
    expect(connectLabel(snapshot('Disabled'))).toBeNull();
    expect(connectLabel(snapshot('Unsupported'))).toBeNull();
    expect(connectLabel(snapshot(null))).toBeNull();
    expect(connectLabel(null)).toBeNull();
  });
});

/** Reached and answered: it asked for time, so there is no button to press,
 * only the reason. Offering Reconnect there makes it worse. tech.md 6.4. */
describe('a rate limited account', () => {
  const limited = (): UsageSnapshot => ({
    windows: [],
    source: 'Unavailable',
    reason: 'RateLimited',
    fetched_at: 0,
    keychain_granted: true,
  });

  it('offers no button and says why', () => {
    expect(connectLabel(limited())).toBeNull();
    expect(reasonText(limited())).toBe('too many requests, it asked to wait');
  });

  /// Reached, throttled, and already connected once: retrying is the wrong
  /// move, but hiding the session list over it is the exact fragility this
  /// gate exists to end. tech.md 6.4.
  it('does not gate a session list that was already reachable', () => {
    expect(gateSessions(limited())).toBe(false);
  });
});

describe('gating the session list', () => {
  const snapshot = (reason: UsageUnavailable | null, granted: boolean): UsageSnapshot => ({
    windows: [],
    source: granted && !reason ? 'Account' : 'Unavailable',
    reason,
    fetched_at: 0,
    keychain_granted: granted,
  });

  it('gates on a first run, where there is a button to press', () => {
    expect(gateSessions(snapshot('NotGranted', false))).toBe(true);
    expect(gateSessions(snapshot('Denied', false))).toBe(true);
    expect(gateSessions(snapshot('Offline', false))).toBe(true);
    expect(gateSessions(snapshot('Network', false))).toBe(true);
  });

  /// Disabled and Unsupported offer no button at all: gating behind one would
  /// trap a person who turned usage tracking off on purpose. tech.md 6.4.
  it('never gates a reason with nothing to press', () => {
    expect(gateSessions(snapshot('Disabled', false))).toBe(false);
    expect(gateSessions(snapshot('Unsupported', false))).toBe(false);
  });

  /// Once granted, a later failure of any kind must not hide history that
  /// was already reachable -- the exact flapping this gate exists to end.
  it('never re-gates once access has been granted', () => {
    expect(gateSessions(snapshot('Network', true))).toBe(false);
    expect(gateSessions(snapshot('Offline', true))).toBe(false);
    expect(gateSessions(snapshot(null, true))).toBe(false);
  });

  /// Before the first snapshot arrives, the list opens on nothing rather than
  /// flashing the gate for the instant before get_state answers.
  it('does not gate before anything is known at all', () => {
    expect(gateSessions(null)).toBe(false);
  });
});
