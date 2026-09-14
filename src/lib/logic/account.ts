/**
 * The sign-in window: whether it stands, which screen it shows, and what that
 * screen says. Pure, so the wording and the rules are tests rather than
 * things to read off a screenshot. tech.md 6.16.
 */

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
  switch (screen) {
    case 'install':
      return {
        title: 'Install Claude Code',
        line: 'Peekle runs on top of Claude Code. Install it in Terminal, then come back.',
      };
    case 'update':
      return {
        title: 'Update Claude Code',
        line: account?.version
          ? `Claude Code ${account.version} is too old to sign in from Peekle.`
          : 'This version of Claude Code is too old to sign in from Peekle.',
      };
    case 'signin':
      return {
        title: 'Sign in to Peekle',
        line: 'Peekle uses your Claude account through Claude Code.',
      };
    case 'starting':
      return { title: 'Opening your browser', line: '' };
    case 'waiting':
      return {
        title: 'Continue in your browser',
        line: 'Approve access on claude.ai. Peekle picks it up on its own.',
      };
    case 'finishing':
      return { title: 'Signing in', line: '' };
    case 'done':
      return { title: "You're signed in", line: '' };
    case 'denied':
      return {
        title: 'Access declined',
        line: "Nothing was shared with Peekle. Try again whenever you're ready.",
      };
    case 'failed':
      return {
        title: "Sign-in didn't finish",
        line: signIn.error ?? 'Claude Code did not finish signing in.',
      };
  }
}
