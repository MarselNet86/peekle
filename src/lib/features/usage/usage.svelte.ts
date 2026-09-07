/**
 * Usage state. tech.md 6.4.
 *
 * The island never waits on this. It opens on the last snapshot and a fresh
 * one arrives as an event, because a slow network call must not delay the
 * answer to a hook.
 */

import { commands, events } from '$lib/bridge';
import type { UsageSnapshot } from '$lib/types/generated/UsageSnapshot';
import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';
import type { UsageWindow } from '$lib/types/generated/UsageWindow';

/** Why the bars are empty, in words rather than in an enum name. */
const REASONS: Record<UsageUnavailable, string> = {
  Disabled: 'usage is off in the config',
  NotGranted: 'needs Keychain access',
  Denied: 'Keychain access was denied',
  NotLoggedIn: 'log in with the Claude Code CLI',
  Offline: 'no internet connection',
  Network: 'could not reach the API',
  RateLimited: 'too many requests, it asked to wait',
  Unsupported: 'the API stopped reporting it',
};

export const WINDOW_LABELS: Record<UsageWindow, string> = {
  FiveHour: '5h',
  SevenDay: 'Week',
};

export function reasonText(snapshot: UsageSnapshot | null): string {
  if (!snapshot?.reason) return 'unavailable';
  return REASONS[snapshot.reason] ?? 'unavailable';
}

/** The two bars to draw, in the order of 6.3, dashes included. */
export function bars(
  snapshot: UsageSnapshot | null,
): { label: string; pct: number | null; resetsAt: number | null }[] {
  const order: UsageWindow[] = ['FiveHour', 'SevenDay'];

  return order.map((window) => {
    const stat = snapshot?.windows.find((w) => w.window === window);
    return {
      label: WINDOW_LABELS[window],
      // No stat means dashes. A number assembled from a missing header would
      // be a guess, and the headers are undocumented. tech.md R-3.
      pct: stat ? stat.used_pct : null,
      resetsAt: stat?.resets_at ?? null,
    };
  });
}

/**
 * The label on the connect control, or null when there is nothing pressing it
 * would fix.
 *
 * `Disabled` is a config switch and `Unsupported` is the API changing shape:
 * offering a button for either sends the user to press something that cannot
 * help. The other four are all one press away from working again, which is why
 * a dropped session and a first run share the same control. tech.md 6.4.
 */
export function connectLabel(snapshot: UsageSnapshot | null): string | null {
  switch (snapshot?.reason) {
    case 'NotGranted':
    case 'Denied':
      return 'Connect';
    case 'NotLoggedIn':
    case 'Offline':
    case 'Network':
      return 'Reconnect';
    // Reached and answered: pressing again is what it asked us not to do, so
    // there is no button, only the reason. tech.md 6.4.
    case 'RateLimited':
      return null;
    default:
      return null;
  }
}

/**
 * Whether the session list should be replaced by one big connect screen
 * instead of drawn at all.
 *
 * Exactly when access has never been granted and there is a button that could
 * fix that -- never for `Disabled`, `Unsupported` or `RateLimited`, where no
 * press would do anything, and never once `keychain_granted` is true, because
 * a later network blip or rate limit must not hide history that was already
 * reachable. Before the first snapshot arrives (`null`) this reads as
 * reachable too, so the list opens on nothing rather than flashing this
 * screen for the instant before `get_state` answers. tech.md 6.4.
 */
export function gateSessions(snapshot: UsageSnapshot | null): boolean {
  return !(snapshot?.keychain_granted ?? true) && connectLabel(snapshot) !== null;
}

export function createUsage() {
  let snapshot = $state<UsageSnapshot | null>(null);
  let connecting = $state(false);

  async function start(): Promise<() => void> {
    const off = await events.onUsage((next) => {
      snapshot = next;
    });

    // A late mount must not drop what Rust already knows, and before the first
    // poll that is the only thing there is to show.
    const state = await commands.getState();
    if (state && !snapshot) snapshot = state.usage;
    return off;
  }

  /**
   * The one path allowed to raise the Keychain dialog, and it exists only
   * because the user pressed something. tech.md 6.4 and rule 12.
   *
   * The same press covers a first connect and a session that dropped: both end
   * with reading the Keychain again and asking the endpoint again.
   */
  async function connect() {
    connecting = true;
    try {
      const next = await commands.requestUsageAccess();
      if (next) snapshot = next;
    } finally {
      connecting = false;
    }
  }

  return {
    get snapshot() {
      return snapshot;
    },
    get bars() {
      return bars(snapshot);
    },
    get reason() {
      return reasonText(snapshot);
    },
    get connectLabel() {
      return connectLabel(snapshot);
    },
    get connecting() {
      return connecting;
    },
    /** Whether the last snapshot came back without numbers at all. */
    get failed() {
      return snapshot !== null && snapshot.reason !== null;
    },
    /** Whether access has ever been granted. tech.md 6.4. */
    get keychainGranted() {
      return snapshot?.keychain_granted ?? false;
    },
    /** Whether the session list should be gated behind one big connect
     * screen rather than drawn. tech.md 6.4. */
    get gateSessions() {
      return gateSessions(snapshot);
    },
    connect,
    start,
  };
}
