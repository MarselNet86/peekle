/**
 * The sign-in window: whether it stands, which screen it shows, and what that
 * screen says. Pure, so the wording and the rules are tests rather than
 * things to read off a screenshot. tech.md 6.16.
 */

import { ACCOUNT } from '$lib/i18n/account';
import { copy } from '$lib/i18n/index.svelte';
import type { AccountState } from '$lib/types/generated/AccountState';
import type { SignInState } from '$lib/types/generated/SignInState';

/** Every screen the window can show, in the order of the table in 6.16. */
export type AuthScreen =
  | 'install'
  | 'update'
  | 'signin'
  | 'starting'
  | 'waiting'
  | 'finishing'
  | 'done'
  | 'denied'
  | 'failed';

/** How long the tick stays on screen after a sign-in went through. A window
 * that vanishes in the moment of success reads as a fault. tech.md 6.16. */
export const DONE_HOLD = 1100;

/** How often the island asks the CLI again while it shows the install or the
 * update screen: the person is in a terminal, and the window should have
 * changed by the time they are back. tech.md 6.16. */
export const ACCOUNT_POLL = 4000;

/**
 * Whether the island shows the sign-in window instead of the list and chats.
 *
 * Only on something the CLI actually said. Nothing known yet is no window --
 * otherwise it would flash on every start -- and `signed_in: null` is "do not
 * know", never "signed out". A hook that is waiting stands in front of
 * everything: answering it is what the island is for. tech.md 6.16.
 */
export function needsAuth(account: AccountState | null, hasPrompt: boolean): boolean {
  if (!account || hasPrompt) return false;
  return account.cli !== 'Ready' || account.signed_in === false;
}

/** Which screen stands. The CLI's own state comes first: a login cannot run
 * without a CLI that can do one. tech.md 6.16. */
export function authScreen(account: AccountState | null, signIn: SignInState): AuthScreen {
  if (account?.cli === 'Missing') return 'install';
  if (account?.cli === 'Outdated') return 'update';
  switch (signIn.stage) {
    case 'Starting':
      return 'starting';
    case 'Waiting':
      return 'waiting';
    case 'Finishing':
      return 'finishing';
    case 'Done':
      return 'done';
    case 'Denied':
      return 'denied';
    case 'Failed':
      return 'failed';
    default:
      return 'signin';
  }
}

/** Whether a screen is waiting on something the person does in a terminal. */
export function polls(screen: AuthScreen): boolean {
  return screen === 'install' || screen === 'update';
}

/** The words of each screen: a title, and a line or nothing. tech.md 6.16. */
export function authCopy(
  screen: AuthScreen,
  account: AccountState | null,
  signIn: SignInState,
): { title: string; line: string } {
  const t = copy(ACCOUNT);
  switch (screen) {
    case 'install':
      return {
        title: t.installTitle,
        line: t.installLine,
      };
    case 'update':
      return {
        title: t.updateTitle,
        line: account?.version ? t.tooOld(account.version) : t.tooOldUnknown,
      };
    case 'signin':
      return {
        title: t.signinTitle,
        line: t.signinLine,
      };
    case 'starting':
      return { title: t.startingTitle, line: '' };
    case 'waiting':
      return {
        title: t.waitingTitle,
        line: t.waitingLine,
      };
    case 'finishing':
      return { title: t.finishingTitle, line: '' };
    case 'done':
      return { title: t.doneTitle, line: '' };
    case 'denied':
      return {
        title: t.deniedTitle,
        line: t.deniedLine,
      };
    case 'failed':
      return {
        title: t.failedTitle,
        // The error is Rust's, already in the language of the interface. 6.28.
        line: signIn.error ?? t.failedLine,
      };
  }
}
