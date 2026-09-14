/**
 * What Claude Code on this Mac can do for the account. tech.md 6.16.
 *
 * Rust asks the CLI; this holds the answer, the press behind `Check again`,
 * and the two seconds `Copy` says `Copied` for.
 */

import { commands, events } from '$lib/bridge';
import type { AccountLink } from '$lib/types/generated/AccountLink';
import type { AccountState } from '$lib/types/generated/AccountState';

/** How long the copy button says it copied. tech.md 6.16. */
export const COPIED_FOR = 2000;

export function createAccount() {
  let state = $state<AccountState | null>(null);
  let checking = $state(false);
  let copied = $state(false);
  let signingOut = $state(false);
  let signOutError = $state<string | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function start(): Promise<() => void> {
    const off = await events.onAccount((next) => {
      state = next;
    });
    // A late mount must not drop what Rust already knows.
    const known = await commands.getAccount();
    if (known && !state) state = known;
    return () => {
      clearTimeout(copiedTimer);
      off();
    };
  }

  /** Asks the CLI again. `quiet` for the asks nobody pressed for: the island
   * opening, the poll. Only a press shows the button turning. */
  async function refresh(options: { quiet?: boolean } = {}) {
    if (!options.quiet) checking = true;
    try {
      const next = await commands.refreshAccount();
      if (next) state = next;
    } finally {
      if (!options.quiet) checking = false;
    }
  }

  async function copy() {
    try {
      const text = await commands.copyAccountCommand();
      if (text === null) return;
      copied = true;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), COPIED_FOR);
    } catch {
      copied = false;
    }
  }

  /** Signs Claude Code out. True when it went through. tech.md 6.16. */
  async function signOut(): Promise<boolean> {
    signingOut = true;
    signOutError = null;
    try {
      const next = await commands.signOut();
      if (next) state = next;
      return true;
    } catch (err) {
      signOutError = String(err);
      return false;
    } finally {
      signingOut = false;
    }
  }

  async function openLink(link: AccountLink) {
    try {
      await commands.openAccountLink(link);
    } catch {
      // The browser did not open; the screen still carries the command.
    }
  }

  return {
    get state() {
      return state;
    },
    get checking() {
      return checking;
    },
    get copied() {
      return copied;
    },
    get signingOut() {
      return signingOut;
    },
    get signOutError() {
      return signOutError;
    },
    signOut,
    start,
    refresh,
    copy,
    openLink,
  };
}
