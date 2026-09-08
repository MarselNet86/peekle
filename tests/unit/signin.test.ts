/**
 * S20 acceptance. `NotLoggedIn` used to end at a Reconnect button that
 * re-read the Keychain -- which cannot fix a credential that is missing or
 * expired -- beside a caption telling the user to go and log in with the CLI.
 * The way back is now in the island. tech.md 6.16.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { commands, events } from '$lib/bridge';
import { createSignIn } from '$lib/features/signin/signin.svelte';
import {
  connectLabel,
  gateSessions,
  needsSignIn,
  outOfReach,
} from '$lib/features/usage/usage.svelte';
import { accountCopy, runCopy } from '$lib/logic/signin';
import { barred } from '$lib/logic/sessions';
import SignInPanel from '$lib/ui/SignInPanel.svelte';
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
});

beforeEach(() => {
  vi.mocked(commands.startSignIn)
    .mockClear()
    .mockResolvedValue(state({ stage: 'Starting' }));
  vi.mocked(commands.submitSignInCode)
    .mockClear()
    .mockResolvedValue(state({ stage: 'Done' }));
  vi.mocked(commands.openSignInPage).mockClear().mockResolvedValue(undefined);
  vi.mocked(commands.cancelSignIn).mockClear().mockResolvedValue(undefined);
  vi.mocked(events.onSignIn)
    .mockClear()
    .mockResolvedValue(() => {});
});

describe('which control a failure earns', () => {
  /// The whole fault. Re-reading the Keychain answers the same way however
  /// many times it is asked, so offering it was a press that could not work.
  it('offers a sign-in for a missing credential, never a reconnect', () => {
    expect(connectLabel(snapshot('NotLoggedIn'))).toBeNull();
    expect(needsSignIn(snapshot('NotLoggedIn'))).toBe(true);
  });

  /// A dropped network really is one press from working again, and a login
  /// would need the network it does not have.
  it('keeps the reconnect for a network that dropped', () => {
    expect(connectLabel(snapshot('Offline'))).toBe('Reconnect');
    expect(connectLabel(snapshot('Network'))).toBe('Reconnect');
    expect(needsSignIn(snapshot('Offline'))).toBe(false);
  });

  it('still says Connect on a first grant', () => {
    expect(connectLabel(snapshot('NotGranted'))).toBe('Connect');
    expect(connectLabel(snapshot('Denied'))).toBe('Connect');
  });

  /// Nothing to press, so nothing is offered. tech.md 6.4.
  it('offers neither where no press would help', () => {
    for (const reason of ['Disabled', 'Unsupported', 'RateLimited'] as const) {
      expect(connectLabel(snapshot(reason))).toBeNull();
      expect(needsSignIn(snapshot(reason))).toBe(false);
    }
  });

  /// The gate stands on "there is a way out of this screen". Moving the login
  /// off `connectLabel` must not leave a signed-out user looking at an empty
  /// list with no control at all.
  it('still gates the session list when the way out is a sign-in', () => {
    expect(gateSessions(snapshot('NotLoggedIn'))).toBe(true);
    expect(gateSessions(snapshot('NotLoggedIn', true))).toBe(false);
    expect(gateSessions(snapshot('RateLimited'))).toBe(false);
  });
});

describe('the panel', () => {
  it('starts the sign-in on the press', async () => {
    const onaction = vi.fn();
    render(SignInPanel, { props: { signIn: state(), reason: 'NotLoggedIn', onaction } });

    await userEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(onaction).toHaveBeenCalledOnce();
  });

  /// A login needs the network it does not have, so the press retries the
  /// read instead. Same screen, different action, and the label says which.
  it('offers a retry rather than a login when the network is gone', async () => {
    const onaction = vi.fn();
    render(SignInPanel, { props: { signIn: state(), reason: 'Offline', onaction } });

    expect(screen.getByText('No connection')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Sign in' })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onaction).toHaveBeenCalledOnce();
  });

  /// The browser not opening is ordinary: another default browser, a refused
  /// `open`. Without a way back to the page there is nowhere left to go.
  /// It is a button and not a link on purpose -- an anchor in an overlay
  /// webview would navigate the overlay itself. tech.md 6.16.
  it('offers the page again once the CLI has printed it', async () => {
    const onopen = vi.fn();
    render(SignInPanel, {
      props: {
        signIn: state({ stage: 'Waiting', url: 'https://claude.com/cai/oauth/authorize?x=1' }),
        onopen,
      },
    });

    expect(screen.getByText('Opening your browser')).toBeInTheDocument();
    expect(screen.queryByRole('link')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Open the page' }));
    expect(onopen).toHaveBeenCalledOnce();
  });

  /// Nothing to open before the CLI has printed an address.
  it('offers nothing to open before there is an address', () => {
    render(SignInPanel, { props: { signIn: state({ stage: 'Starting' }) } });
    expect(screen.queryByText('Open the page')).not.toBeInTheDocument();
  });

  it('sends the code on Enter', async () => {
    const oncode = vi.fn();
    render(SignInPanel, {
      props: { signIn: state({ stage: 'Waiting', needs_code: true }), oncode },
    });

    await userEvent.type(screen.getByLabelText('Code from the page'), 'abc123{Enter}');
    expect(oncode).toHaveBeenCalledExactlyOnceWith('abc123');
  });

  /// An empty code would put the CLI's own prompt through a round trip that
  /// answers nothing.
  it('does not send an empty code', async () => {
    const oncode = vi.fn();
    render(SignInPanel, {
      props: { signIn: state({ stage: 'Waiting', needs_code: true }), oncode },
    });

    await userEvent.type(screen.getByLabelText('Code from the page'), '   {Enter}');
    expect(oncode).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
  });

  it('cancels while it is running', async () => {
    const oncancel = vi.fn();
    render(SignInPanel, { props: { signIn: state({ stage: 'Waiting' }), oncancel } });

    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(oncancel).toHaveBeenCalledOnce();
  });

  /// A press that changes nothing on screen reads as a press that was lost --
  /// which is what made the old button feel broken. tech.md 6.4.
  it('says why it failed, in words', () => {
    render(SignInPanel, {
      props: {
        signIn: state({ stage: 'Failed', error: 'No claude command found on this Mac.' }),
        reason: 'NotLoggedIn',
      },
    });

    expect(screen.getByText('No claude command found on this Mac.')).toBeInTheDocument();
    // And it is pressable again: a dead end is not an outcome.
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeEnabled();
  });
});

describe('the store', () => {
  it('carries the stage back from the press', async () => {
    const signIn = createSignIn();
    await signIn.begin();

    expect(commands.startSignIn).toHaveBeenCalledOnce();
    expect(signIn.state.stage).toBe('Starting');
    expect(signIn.open).toBe(true);
    expect(signIn.busy).toBe(false);
  });

  it('never sends an empty code to Rust', async () => {
    const signIn = createSignIn();
    await signIn.submit('   ');

    expect(commands.submitSignInCode).not.toHaveBeenCalled();
  });

  /// Cancelling closes the panel on the press, not a round trip later.
  it('closes on cancel and tells Rust to end the process', async () => {
    const signIn = createSignIn();
    await signIn.begin();
    await signIn.cancel();

    expect(signIn.state.stage).toBe('Idle');
    expect(signIn.open).toBe(false);
    expect(commands.cancelSignIn).toHaveBeenCalledOnce();
  });

  /// Every stage says something: a panel that goes blank between the press
  /// and the address is the lost press this path exists to fix.
  it('has words for every stage a process is up in', async () => {
    const signIn = createSignIn();
    const stop = await signIn.start();
    const [handler] = vi.mocked(events.onSignIn).mock.calls.at(-1) ?? [];
    if (!handler) throw new Error('nothing subscribed to the sign-in event');

    for (const stage of ['Starting', 'Waiting', 'Finishing'] as const) {
      handler(state({ stage }));
      expect(signIn.open).toBe(true);
    }

    handler(state({ stage: 'Done' }));
    expect(signIn.open).toBe(false);
    stop();
  });
});

describe('a chat opened while the account is out of reach', () => {
  /// A conversation the island cannot reach is a dead screen with a
  /// scrollbar, so the way back in stands where the chat would be.
  it('shows the sign-in instead of the chat', () => {
    expect(barred({ outOfReach: true, hasPrompt: false })).toBe(true);
  });

  /// The one thing the island exists for. A hook waiting on Allow or Deny is
  /// blocking a real agent run, and no account problem may stand in front of
  /// it. tech.md 6.4.
  it('never stands in front of a hook that is waiting', () => {
    expect(barred({ outOfReach: true, hasPrompt: true })).toBe(false);
  });

  it('is out of the way whenever the account is reachable', () => {
    expect(barred({ outOfReach: false, hasPrompt: false })).toBe(false);
    expect(barred({ outOfReach: false, hasPrompt: true })).toBe(false);
  });
});

describe('what the account screen says', () => {
  /// The bug it closes: with the network gone, a send into an observed chat
  /// came back "that chat is busy elsewhere". It named the wrong problem and
  /// sent the reader hunting for another client to close. tech.md 6.16.
  it('bars a chat when Anthropic is unreachable, whichever way', () => {
    expect(outOfReach(snapshot('Offline'))).toBe(true);
    expect(outOfReach(snapshot('Network'))).toBe(true);
    expect(outOfReach(snapshot('NotLoggedIn'))).toBe(true);
  });

  /// Under all of these the agent works, so nothing stands in front of a chat.
  it('leaves a chat alone for anything the agent survives', () => {
    for (const reason of [
      'Disabled',
      'Unsupported',
      'RateLimited',
      'NotGranted',
      'Denied',
    ] as const) {
      expect(outOfReach(snapshot(reason))).toBe(false);
    }
    expect(outOfReach(snapshot(null))).toBe(false);
    expect(outOfReach(null)).toBe(false);
  });

  /// Each reason gets its own words and its own verb. A screen that says the
  /// same thing about a dead network and an expired login teaches nothing.
  it('names the problem and the verb that fixes it', () => {
    expect(accountCopy('Offline')?.action).toBe('Try again');
    expect(accountCopy('Network')?.action).toBe('Try again');
    expect(accountCopy('NotLoggedIn')?.action).toBe('Sign in');

    const titles = (['Offline', 'Network', 'NotLoggedIn'] as const).map(
      (reason) => accountCopy(reason)?.title,
    );
    expect(new Set(titles).size).toBe(3);
  });

  it('says nothing for a reason no screen should stand on', () => {
    expect(accountCopy('RateLimited')).toBeNull();
    expect(accountCopy(null)).toBeNull();
  });

  /// Short enough to read at a glance, on a screen that is 300px wide.
  it('keeps every line short', () => {
    for (const reason of ['Offline', 'Network', 'NotLoggedIn'] as const) {
      const copy = accountCopy(reason);
      expect(copy!.title.length).toBeLessThanOrEqual(20);
      expect(copy!.line.length).toBeLessThanOrEqual(48);
    }
  });

  it('has a line for every stage a process is up in, and none otherwise', () => {
    for (const stage of ['Starting', 'Waiting', 'Finishing'] as const) {
      expect(runCopy(state({ stage }))?.title.length).toBeGreaterThan(0);
    }
    expect(runCopy(state({ stage: 'Idle' }))).toBeNull();
    expect(runCopy(state({ stage: 'Done' }))).toBeNull();
    expect(runCopy(state({ stage: 'Failed' }))).toBeNull();
  });
});
