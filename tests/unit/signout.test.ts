/**
 * v83.1 acceptance: signing out from the settings. tech.md 6.16.
 *
 * Criteria, from the owner's request: the settings offer a way out of the
 * Claude session, and after it the island shows the sign-in window. Because
 * the sign-out is Claude Code's own and signs the terminal out too, one press
 * asks and only the second one does it.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { commands } from '$lib/bridge';
import { createAccount } from '$lib/features/account/account.svelte';
import ActionRow from '$lib/ui/ActionRow.svelte';
import type { AccountState } from '$lib/types/generated/AccountState';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return { ...real, commands: { ...real.commands, signOut: vi.fn() } };
});

const CONFIRM = 'Signs Claude Code out on this Mac, the terminal included.';
const signedOut: AccountState = {
  cli: 'Ready',
  version: '2.1.263',
  install: 'Homebrew',
  signed_in: false,
  command: null,
};

const row = (props: Record<string, unknown> = {}) =>
  render(ActionRow, {
    props: {
      label: 'Claude account',
      hint: 'Signed in to Claude Code on this Mac.',
      action: 'Sign out',
      confirm: CONFIRM,
      ...props,
    },
  });

beforeEach(() => {
  vi.mocked(commands.signOut).mockReset().mockResolvedValue(signedOut);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('the sign-out row', () => {
  it('asks on the first press and does nothing yet', async () => {
    const onaction = vi.fn();
    row({ onaction });

    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
    expect(onaction).not.toHaveBeenCalled();
    expect(screen.getByText(CONFIRM)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
  });

  it('signs out on the second press', async () => {
    const onaction = vi.fn();
    row({ onaction });

    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
    expect(onaction).toHaveBeenCalledOnce();
    expect(screen.queryByText(CONFIRM)).not.toBeInTheDocument();
  });

  it('takes the question back on Cancel', async () => {
    const onaction = vi.fn();
    row({ onaction });

    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.getByText('Signed in to Claude Code on this Mac.')).toBeInTheDocument();

    // And the next press asks again rather than signing out.
    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }));
    expect(onaction).not.toHaveBeenCalled();
  });

  /// A row left alone is never found asking.
  it('takes the question back by itself', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onaction = vi.fn();
    row({ onaction });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    await user.click(screen.getByRole('button', { name: 'Sign out' }));
    expect(screen.getByText(CONFIRM)).toBeInTheDocument();
    vi.advanceTimersByTime(4100);
    await vi.waitFor(() => expect(screen.queryByText(CONFIRM)).not.toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Sign out' }));
    expect(onaction).not.toHaveBeenCalled();
  });

  it('says why a sign-out did not go through', () => {
    row({ error: 'Claude Code could not sign out.' });
    expect(screen.getByText('Claude Code could not sign out.')).toBeInTheDocument();
  });
});

describe('the account store', () => {
  it('takes the fresh account, which is what raises the window', async () => {
    const store = createAccount();
    expect(await store.signOut()).toBe(true);
    expect(commands.signOut).toHaveBeenCalledOnce();
    expect(store.state).toEqual(signedOut);
    expect(store.signingOut).toBe(false);
    expect(store.signOutError).toBeNull();
  });

  it('keeps the account and says why when it did not go through', async () => {
    vi.mocked(commands.signOut).mockRejectedValueOnce('Claude Code could not sign out.');
    const store = createAccount();

    expect(await store.signOut()).toBe(false);
    expect(store.state).toBeNull();
    expect(store.signOutError).toBe('Claude Code could not sign out.');
  });
});
