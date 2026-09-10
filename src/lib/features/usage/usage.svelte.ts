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
  NotLoggedIn: 'not signed in',
  Offline: 'no internet connection',
  Network: 'could not reach the API',
  RateLimited: 'too many requests, it asked to wait',
  Unsupported: 'the API stopped reporting it',
};

export const WINDOW_LABELS: Record<UsageWindow, string> = {
  FiveHour: '5h',
  SevenDay: 'Week',
  // A stand-in only. The scoped window is named by the server, and the name
  // it gave is what the dial wears. tech.md 6.4.
  SevenDayScoped: 'Model',
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
 * The week a plan counts for one model on its own, or null when the plan has
 * no such window.
 *
 * Null is the whole point: this one is not part of the contract of two, and a
 * dash where the plan has no window at all would promise a number that is
 * never coming. It wears the name the server gave it, because Peekle does not
 * know which models a plan counts apart. tech.md 6.4.
 */
export function scopedBar(
  snapshot: UsageSnapshot | null,
): { label: string; pct: number; resetsAt: number | null } | null {
  const stat = snapshot?.windows.find((w) => w.window === 'SevenDayScoped');
  if (!stat) return null;

  // A window with no name has nothing to write under the dial, and a dial
  // labelled after a guess is worse than no dial.
  const label = stat.scope?.trim();
  if (!label) return null;

  return { label, pct: stat.used_pct, resetsAt: stat.resets_at };
}

/**
 * The label on the connect control, or null when there is nothing pressing it
 * would fix.
 *
 * `Disabled` is a config switch and `Unsupported` is the API changing shape:
 * offering a button for either sends the user to press something that cannot
 * help. `RateLimited` was reached and answered, and pressing again is the one
 * thing it asked us not to do. `NotLoggedIn` has a control of its own -- it is
 * the sign-in of 6.16, not this -- because re-reading the Keychain cannot fix
 * a credential that is missing or expired, and offering it here was the whole
 * reason Reconnect earned its reputation. What is left is a first grant and a
 * network that dropped, and both really are one press from working again.
 * tech.md 6.4.
 */
export function connectLabel(snapshot: UsageSnapshot | null): string | null {
  switch (snapshot?.reason) {
    case 'NotGranted':
    case 'Denied':
      return 'Connect';
    case 'Offline':
    case 'Network':
      return 'Reconnect';
    default:
      return null;
  }
}

/**
 * Whether this is a sign-in problem rather than anything a reconnect touches.
 *
 * The one reason `claude auth login` is the answer to. tech.md 6.16.
 */
export function needsSignIn(snapshot: UsageSnapshot | null): boolean {
  return snapshot?.reason === 'NotLoggedIn';
}

/**
 * Whether the island cannot reach Anthropic at all.
 *
 * An agent needs the same network and the same account the bars do, so these
 * three mean a message typed now goes nowhere useful. The rest do not: usage
 * switched off, an endpoint that changed shape, a rate limit that clears
 * itself, a Keychain grant nobody has given -- under every one of those the
 * agent works and the chat stays open. tech.md 6.16.
 */
export function outOfReach(snapshot: UsageSnapshot | null): boolean {
  const reason = snapshot?.reason;
  return reason === 'NotLoggedIn' || reason === 'Offline' || reason === 'Network';
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
  return (
    !(snapshot?.keychain_granted ?? true) &&
    (connectLabel(snapshot) !== null || needsSignIn(snapshot))
  );
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
    /** The model window, when the plan has one. tech.md 6.4. */
    get scoped() {
      return scopedBar(snapshot);
    },
    get reason() {
      return reasonText(snapshot);
    },
    get connectLabel() {
      return connectLabel(snapshot);
    },
    /** Whether the way back is a sign-in rather than a reconnect. 6.16. */
    get needsSignIn() {
      return needsSignIn(snapshot);
    },
    /** Whether Anthropic is unreachable, so an agent cannot work. 6.16. */
    get outOfReach() {
      return outOfReach(snapshot);
    },
    get reasonCode() {
      return snapshot?.reason ?? null;
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
