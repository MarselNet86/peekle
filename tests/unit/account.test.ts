/**
 * v83 acceptance: the sign-in window. tech.md 6.16.
 *
 * Criteria, from the owner's request: no Claude Code means the install
 * command with a way to copy it; an old one means "update" with links; a
 * press on sign in goes to the browser, and the island picks up Allow or Deny
 * by itself; the page may show a code instead, and the code goes in here; a
 * signed-out person sees the window and nothing of their history.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { commands, events } from '$lib/bridge';
import { COPIED_FOR, createAccount } from '$lib/features/account/account.svelte';
import { authCopy, authScreen, needsAuth, polls, type AuthScreen } from '$lib/logic/account';
import AuthPanel from '$lib/ui/AuthPanel.svelte';
import CommandLine from '$lib/ui/CommandLine.svelte';
import type { AccountState } from '$lib/types/generated/AccountState';
import type { SignInState } from '$lib/types/generated/SignInState';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: {
      ...real.commands,
      getAccount: vi.fn(),
      refreshAccount: vi.fn(),
      copyAccountCommand: vi.fn(),
      openAccountLink: vi.fn(),
    },
    events: { ...real.events, onAccount: vi.fn() },
  };
});

const INSTALL = 'curl -fsSL https://claude.ai/install.sh | bash';

const account = (over: Partial<AccountState> = {}): AccountState => ({
  cli: 'Ready',
  version: '2.1.263',
  install: 'Homebrew',
  signed_in: false,
  command: null,
  ...over,
});
const missing = account({
  cli: 'Missing',
  version: null,
  install: 'Unknown',
  signed_in: null,
  command: INSTALL,
});
const outdated = account({
  cli: 'Outdated',
  version: '2.0.14',
  signed_in: null,
  command: 'brew upgrade --cask claude-code@latest',
});

const run = (over: Partial<SignInState> = {}): SignInState => ({
  stage: 'Idle',
  url: null,
  needs_code: false,
  error: null,
  ...over,
});

beforeEach(() => {
  vi.mocked(commands.getAccount).mockReset().mockResolvedValue(null);
  vi.mocked(commands.refreshAccount).mockReset().mockResolvedValue(account());
  vi.mocked(commands.copyAccountCommand).mockReset().mockResolvedValue(INSTALL);
  vi.mocked(commands.openAccountLink).mockReset().mockResolvedValue(undefined);
  vi.mocked(events.onAccount)
    .mockReset()
    .mockResolvedValue(() => {});
});

afterEach(() => {
  vi.useRealTimers();
});

describe('when the window stands', () => {
  it('stands for a signed-out account, a missing CLI and an old one', () => {
    expect(needsAuth(account({ signed_in: false }), false)).toBe(true);
    expect(needsAuth(missing, false)).toBe(true);
    expect(needsAuth(outdated, false)).toBe(true);
  });

  it('does not stand for a signed-in account', () => {
    expect(needsAuth(account({ signed_in: true }), false)).toBe(false);
  });

  /// "Do not know" is never "signed out", and nothing known yet is no
  /// window: otherwise it would flash on every start.
  it('does not stand on a guess', () => {
    expect(needsAuth(account({ signed_in: null }), false)).toBe(false);
    expect(needsAuth(null, false)).toBe(false);
  });

  /// Answering a waiting hook is what the island is for.
  it('never stands in front of a waiting hook', () => {
    expect(needsAuth(missing, true)).toBe(false);
    expect(needsAuth(account({ signed_in: false }), true)).toBe(false);
  });
});

describe('which screen stands', () => {
  it('puts the CLI first', () => {
    expect(authScreen(missing, run({ stage: 'Waiting' }))).toBe('install');
    expect(authScreen(outdated, run({ stage: 'Done' }))).toBe('update');
  });

  it('follows the run', () => {
    const cases: [SignInState['stage'], AuthScreen][] = [
      ['Idle', 'signin'],
      ['Starting', 'starting'],
      ['Waiting', 'waiting'],
      ['Finishing', 'finishing'],
      ['Done', 'done'],
      ['Denied', 'denied'],
      ['Failed', 'failed'],
    ];
    for (const [stage, expected] of cases) {
      expect(authScreen(account(), run({ stage }))).toBe(expected);
    }
    expect(authScreen(null, run())).toBe('signin');
  });

  it('keeps asking only while a terminal is what the person is in', () => {
    expect(polls('install')).toBe(true);
    expect(polls('update')).toBe(true);
    for (const screen of ['signin', 'waiting', 'done', 'denied', 'failed'] as AuthScreen[]) {
      expect(polls(screen)).toBe(false);
    }
  });

  it('names the version that is too old, and does without one', () => {
    expect(authCopy('update', outdated, run()).line).toContain('2.0.14');
    expect(authCopy('update', account({ version: null }), run()).line).toContain('This version');
  });

  it('says why a run failed, in the words Rust gave', () => {
    expect(authCopy('failed', account(), run({ error: 'Invalid code' })).line).toBe('Invalid code');
    expect(authCopy('failed', account(), run()).line.length).toBeGreaterThan(0);
  });
});

describe('the window', () => {
  it('asks to install, with the command and a way to copy it', async () => {
    const oncopy = vi.fn();
    const onlink = vi.fn();
    const oncheck = vi.fn();
    render(AuthPanel, { props: { account: missing, signIn: run(), oncopy, onlink, oncheck } });

    expect(screen.getByRole('heading', { name: 'Install Claude Code' })).toBeInTheDocument();
    expect(screen.getByText(INSTALL)).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Copy' }));
    expect(oncopy).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole('button', { name: 'Installation guide' }));
    expect(onlink).toHaveBeenCalledExactlyOnceWith('InstallGuide');
    await userEvent.click(screen.getByRole('button', { name: 'Check again' }));
    expect(oncheck).toHaveBeenCalledOnce();
    expect(screen.queryByRole('button', { name: 'Sign in with Claude' })).not.toBeInTheDocument();
  });

  it('says Copied once the command is on the pasteboard', () => {
    render(AuthPanel, { props: { account: missing, signIn: run(), copied: true } });
    expect(screen.getByRole('button', { name: 'Copied' })).toBeInTheDocument();
  });

  it('asks to update, with the command for this install and both links', async () => {
    const onlink = vi.fn();
    render(AuthPanel, { props: { account: outdated, signIn: run(), onlink } });

    expect(screen.getByRole('heading', { name: 'Update Claude Code' })).toBeInTheDocument();
    expect(screen.getByText('brew upgrade --cask claude-code@latest')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: "What's new" }));
    await userEvent.click(screen.getByRole('button', { name: 'Update guide' }));
    expect(onlink.mock.calls).toEqual([['Changelog'], ['UpdateGuide']]);
  });

  it('offers one thing to a signed-out account: signing in', async () => {
    const onsignin = vi.fn();
    render(AuthPanel, { props: { account: account(), signIn: run(), onsignin } });

    expect(screen.getByRole('heading', { name: 'Sign in to Peekle' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Sign in with Claude' }));
    expect(onsignin).toHaveBeenCalledOnce();
  });

  /// The browser answers by itself, so nothing here asks for a code until the
  /// person says they have one.
  it('waits on the browser without asking for a code', () => {
    render(AuthPanel, {
      props: {
        account: account(),
        signIn: run({
          stage: 'Waiting',
          url: 'https://claude.com/cai/oauth/authorize',
          needs_code: true,
        }),
      },
    });

    expect(screen.getByRole('heading', { name: 'Continue in your browser' })).toBeInTheDocument();
    expect(screen.getByRole('status', { name: 'Waiting for your browser' })).toBeInTheDocument();
    expect(screen.queryByLabelText('Paste the code')).not.toBeInTheDocument();
  });

  it('takes the code the page showed, on Enter', async () => {
    const oncode = vi.fn();
    render(AuthPanel, {
      props: { account: account(), signIn: run({ stage: 'Waiting', needs_code: true }), oncode },
    });

    await userEvent.click(screen.getByRole('button', { name: 'Have a code?' }));
    await userEvent.type(screen.getByLabelText('Paste the code'), '  abc#123  {Enter}');
    expect(oncode).toHaveBeenCalledExactlyOnceWith('abc#123');
  });

  it('does not send an empty code', async () => {
    const oncode = vi.fn();
    render(AuthPanel, {
      props: { account: account(), signIn: run({ stage: 'Waiting', needs_code: true }), oncode },
    });

    await userEvent.click(screen.getByRole('button', { name: 'Have a code?' }));
    await userEvent.type(screen.getByLabelText('Paste the code'), '   {Enter}');
    expect(oncode).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeDisabled();
  });

  it('opens the page again and cancels while it waits', async () => {
    const onopen = vi.fn();
    const oncancel = vi.fn();
    render(AuthPanel, {
      props: {
        account: account(),
        signIn: run({ stage: 'Waiting', url: 'https://claude.com/cai/oauth/authorize' }),
        onopen,
        oncancel,
      },
    });

    // A button, never a link: an anchor would navigate the overlay itself.
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Open the page again' }));
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onopen).toHaveBeenCalledOnce();
    expect(oncancel).toHaveBeenCalledOnce();
  });

  it('offers nothing to open before there is an address', () => {
    render(AuthPanel, { props: { account: account(), signIn: run({ stage: 'Waiting' }) } });
    expect(screen.queryByRole('button', { name: 'Open the page again' })).not.toBeInTheDocument();
  });

  it('says it is signing in, and then that it did', () => {
    const { unmount } = render(AuthPanel, {
      props: { account: account(), signIn: run({ stage: 'Finishing' }) },
    });
    expect(screen.getByRole('status', { name: 'Signing in' })).toBeInTheDocument();
    unmount();

    render(AuthPanel, { props: { account: account(), signIn: run({ stage: 'Done' }) } });
    expect(screen.getByRole('heading', { name: "You're signed in" })).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  /// Declining is not a fault, and the screen says so rather than calling it
  /// a failure.
  it('tells a refusal from a failure, and offers another go at both', async () => {
    const onsignin = vi.fn();
    const { unmount } = render(AuthPanel, {
      props: { account: account(), signIn: run({ stage: 'Denied' }), onsignin },
    });
    expect(screen.getByRole('heading', { name: 'Access declined' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onsignin).toHaveBeenCalledOnce();
    unmount();

    render(AuthPanel, {
      props: { account: account(), signIn: run({ stage: 'Failed', error: 'Invalid code' }) },
    });
    expect(screen.getByRole('heading', { name: "Sign-in didn't finish" })).toBeInTheDocument();
    expect(screen.getByText('Invalid code')).toBeInTheDocument();
  });

  it('never shows the retired dead end', () => {
    for (const stage of ['Idle', 'Waiting', 'Finishing', 'Done', 'Denied', 'Failed'] as const) {
      const { unmount } = render(AuthPanel, {
        props: { account: account(), signIn: run({ stage }) },
      });
      expect(screen.queryByText(/API refused/)).not.toBeInTheDocument();
      unmount();
    }
  });
});

describe('the command line', () => {
  it('shows the command and copies it on the press', async () => {
    const oncopy = vi.fn();
    render(CommandLine, { props: { command: INSTALL, oncopy } });

    expect(screen.getByText(INSTALL)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Copy' }));
    expect(oncopy).toHaveBeenCalledOnce();
  });
});

describe('the account store', () => {
  it('takes what Rust already knows on mount', async () => {
    vi.mocked(commands.getAccount).mockResolvedValue(missing);
    const store = createAccount();
    await store.start();
    expect(store.state).toEqual(missing);
  });

  it('follows the event', async () => {
    const store = createAccount();
    await store.start();
    const [handler] = vi.mocked(events.onAccount).mock.calls.at(-1) ?? [];
    handler?.(outdated);
    expect(store.state).toEqual(outdated);
  });

  it('turns the check button only for a press', async () => {
    let finish: (value: AccountState) => void = () => {};
    vi.mocked(commands.refreshAccount).mockImplementation(
      () => new Promise((resolve) => (finish = resolve)),
    );
    const store = createAccount();

    const quiet = store.refresh({ quiet: true });
    expect(store.checking).toBe(false);
    finish(account());
    await quiet;

    const pressed = store.refresh();
    expect(store.checking).toBe(true);
    finish(account({ signed_in: true }));
    await pressed;
    expect(store.checking).toBe(false);
    expect(store.state?.signed_in).toBe(true);
  });

  it('says Copied for two seconds, and only after Rust copied', async () => {
    vi.useFakeTimers();
    const store = createAccount();

    await store.copy();
    expect(commands.copyAccountCommand).toHaveBeenCalledOnce();
    expect(store.copied).toBe(true);
    vi.advanceTimersByTime(COPIED_FOR + 1);
    expect(store.copied).toBe(false);

    vi.mocked(commands.copyAccountCommand).mockRejectedValueOnce('there is no command to copy');
    await store.copy();
    expect(store.copied).toBe(false);
  });

  it('names the page and never supplies an address', async () => {
    const store = createAccount();
    await store.openLink('UpdateGuide');
    expect(commands.openAccountLink).toHaveBeenCalledExactlyOnceWith('UpdateGuide');
  });
});
