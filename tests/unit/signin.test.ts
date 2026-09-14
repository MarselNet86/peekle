/**
 * What is left of the old account screen after v83: the network. Signing in
 * moved to the sign-in window (`account.test.ts`), and a usage read that
 * failed while Claude Code is signed in no longer raises any screen at all --
 * the "Signed in, but the API refused" dead end is gone. tech.md 6.16.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { commands, events } from '$lib/bridge';
import { createSignIn } from '$lib/features/signin/signin.svelte';
import { connectLabel, gateSessions, outOfReach } from '$lib/features/usage/usage.svelte';
import { DONE_HOLD } from '$lib/logic/account';
import { reachCopy } from '$lib/logic/reach';
import { barred } from '$lib/logic/sessions';
import ReachPanel from '$lib/ui/ReachPanel.svelte';
import type { SignInState } from '$lib/types/generated/SignInState';
import type { UsageSnapshot } from '$lib/types/generated/UsageSnapshot';
import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: {
      ...real.commands,
      startSignIn: vi.fn(),
      submitSignInCode: vi.fn(),
      openSignInPage: vi.fn(),
      cancelSignIn: vi.fn(),
    },
    events: { ...real.events, onSignIn: vi.fn() },
  };
});

const state = (over: Partial<SignInState> = {}): SignInState => ({
  stage: 'Idle',
  url: null,
  needs_code: false,
  error: null,
  ...over,
});

const snapshot = (reason: UsageUnavailable | null, granted = false): UsageSnapshot => ({
  windows: [],
  source: 'Unavailable',
  reason,
  fetched_at: 0,
  keychain_granted: granted,
  retry_after_ms: null,
});

beforeEach(() => {
  vi.mocked(commands.startSignIn)
    .mockClear()
    .mockResolvedValue(state({ stage: 'Starting' }));
  vi.mocked(commands.submitSignInCode)
    .mockClear()
    .mockResolvedValue(state({ stage: 'Finishing' }));
  vi.mocked(commands.openSignInPage).mockClear().mockResolvedValue(undefined);
  vi.mocked(commands.cancelSignIn).mockClear().mockResolvedValue(undefined);
  vi.mocked(events.onSignIn)
    .mockClear()
    .mockResolvedValue(() => {});
});

afterEach(() => {
  vi.useRealTimers();
});

describe('which control a failure earns', () => {
  /// A dropped network really is one press from working again.
  it('keeps the reconnect for a network that dropped', () => {
    expect(connectLabel(snapshot('Offline'))).toBe('Reconnect');
    expect(connectLabel(snapshot('Network'))).toBe('Reconnect');
  });

  it('still says Connect on a first grant', () => {
    expect(connectLabel(snapshot('NotGranted'))).toBe('Connect');
    expect(connectLabel(snapshot('Denied'))).toBe('Connect');
  });

  /// Nothing to press, so nothing is offered. `NotLoggedIn` is among them
  /// now: re-reading cannot fix it, and signing in is the window's.
  it('offers nothing where no press would help', () => {
    for (const reason of ['Disabled', 'Unsupported', 'RateLimited', 'NotLoggedIn'] as const) {
      expect(connectLabel(snapshot(reason))).toBeNull();
    }
  });

  /// The Keychain gate stands on "there is a button that fixes this".
  it('gates the list only behind a grant a press can give', () => {
    expect(gateSessions(snapshot('NotGranted'))).toBe(true);
    expect(gateSessions(snapshot('NotGranted', true))).toBe(false);
    expect(gateSessions(snapshot('NotLoggedIn'))).toBe(false);
    expect(gateSessions(snapshot('RateLimited'))).toBe(false);
    expect(gateSessions(null)).toBe(false);
  });
});

describe('a chat opened while Anthropic is out of reach', () => {
  /// The regression of the screenshots: the usage endpoint refused a token
  /// while Claude Code was signed in, and the chat was replaced by a screen
  /// with no way out. The agent works in that state, so nothing stands in
  /// front of the chat. tech.md 6.16.
  it('is not barred by a refused usage read', () => {
    expect(outOfReach(snapshot('NotLoggedIn'))).toBe(false);
  });

  it('is barred only when the network is gone', () => {
    expect(outOfReach(snapshot('Offline'))).toBe(true);
    expect(outOfReach(snapshot('Network'))).toBe(true);
    for (const reason of [
      'Disabled',
      'Unsupported',
      'RateLimited',
      'NotGranted',
      'Denied',
    ] as const) {
      expect(outOfReach(snapshot(reason))).toBe(false);
    }
    expect(outOfReach(null)).toBe(false);
  });

  it('never stands in front of a hook that is waiting', () => {
    expect(barred({ outOfReach: true, hasPrompt: true })).toBe(false);
    expect(barred({ outOfReach: true, hasPrompt: false })).toBe(true);
  });
});

describe('what the connection screen says', () => {
  it('names the problem and the verb that fixes it', () => {
    expect(reachCopy('Offline')).toEqual({
      title: 'No connection',
      line: 'Peekle cannot reach Anthropic.',
      action: 'Try again',
    });
    expect(reachCopy('Network')?.title).toBe('No answer');
  });

  /// A login is not a network problem, and says nothing here.
  it('says nothing about signing in', () => {
    expect(reachCopy('NotLoggedIn')).toBeNull();
    expect(reachCopy('RateLimited')).toBeNull();
    expect(reachCopy(null)).toBeNull();
  });

  it('retries on the press, in either form', async () => {
    for (const compact of [false, true]) {
      const onretry = vi.fn();
      const { unmount } = render(ReachPanel, { props: { reason: 'Offline', compact, onretry } });

      expect(screen.getByText('No connection')).toBeInTheDocument();
      await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
      expect(onretry).toHaveBeenCalledOnce();
      unmount();
    }
  });

  it('draws nothing for a reason that is not the network', () => {
    const { container } = render(ReachPanel, { props: { reason: 'NotLoggedIn' } });
    expect(container.textContent?.trim()).toBe('');
  });
});

describe('the sign-in store', () => {
  it('carries the stage back from the press', async () => {
    const signIn = createSignIn();
    await signIn.begin();

    expect(commands.startSignIn).toHaveBeenCalledOnce();
    expect(signIn.state.stage).toBe('Starting');
    expect(signIn.showing).toBe(true);
    expect(signIn.busy).toBe(false);
  });

  it('never sends an empty code to Rust', async () => {
    const signIn = createSignIn();
    await signIn.submit('   ');
    expect(commands.submitSignInCode).not.toHaveBeenCalled();
  });

  /// Cancelling closes the window's run on the press, not a round trip later.
  it('closes on cancel and tells Rust to end the process', async () => {
    const signIn = createSignIn();
    await signIn.begin();
    await signIn.cancel();

    expect(signIn.state.stage).toBe('Idle');
    expect(signIn.showing).toBe(false);
    expect(commands.cancelSignIn).toHaveBeenCalledOnce();
  });

  /// The tick of a sign-in that went through stands a moment, then gives the
  /// island back. tech.md 6.16.
  it('holds the tick after a sign-in, then lets go', async () => {
    vi.useFakeTimers();
    const signIn = createSignIn();
    const stop = await signIn.start();
    const [handler] = vi.mocked(events.onSignIn).mock.calls.at(-1) ?? [];
    if (!handler) throw new Error('nothing subscribed to the sign-in event');

    for (const stage of ['Starting', 'Waiting', 'Finishing'] as const) {
      handler(state({ stage }));
      expect(signIn.showing).toBe(true);
    }

    handler(state({ stage: 'Done' }));
    expect(signIn.showing).toBe(true);
    vi.advanceTimersByTime(DONE_HOLD - 10);
    expect(signIn.showing).toBe(true);
    vi.advanceTimersByTime(20);
    expect(signIn.showing).toBe(false);
    expect(signIn.state.stage).toBe('Idle');
    stop();
  });

  /// A refusal keeps nothing open by itself: it stands only while the
  /// account is signed out, which is the window's own reason to stand.
  it('does not hold the window for a refusal', async () => {
    const signIn = createSignIn();
    const stop = await signIn.start();
    const [handler] = vi.mocked(events.onSignIn).mock.calls.at(-1) ?? [];
    handler?.(state({ stage: 'Denied' }));

    expect(signIn.state.stage).toBe('Denied');
    expect(signIn.showing).toBe(false);
    stop();
  });
});
