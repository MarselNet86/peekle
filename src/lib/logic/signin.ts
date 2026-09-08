/**
 * What the account screen says. Pure, so the wording is a test rather than a
 * thing to read off a screenshot. tech.md 6.16.
 */

import type { SignInState } from '$lib/types/generated/SignInState';
import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

export type AccountCopy = {
  title: string;
  line: string;
  /** What the button does, in the words of the thing it does. */
  action: string;
};

/**
 * Why the island cannot reach the account, and the one thing that fixes it.
 *
 * Null for every reason that does not stop an agent working: usage switched
 * off, an endpoint that changed shape, a rate limit that clears itself, a
 * Keychain grant nobody has given yet. Those belong to the bars, not to a
 * screen that stands in front of a conversation. tech.md 6.4 and 6.16.
 */
export function accountCopy(reason: UsageUnavailable | null | undefined): AccountCopy | null {
  switch (reason) {
    case 'NotLoggedIn':
      return {
        title: 'Signed out',
        line: 'Claude Code needs to sign in again.',
        action: 'Sign in',
      };
    case 'Offline':
      return {
        title: 'No connection',
        line: 'Peekle cannot reach Anthropic.',
        action: 'Try again',
      };
    case 'Network':
      return {
        title: 'No answer',
        line: 'Anthropic did not respond.',
        action: 'Try again',
      };
    default:
      return null;
  }
}

/**
 * What the screen says once a sign-in is actually running.
 *
 * One line per stage and no more. The CLI opens the browser, the page shows a
 * code, the code goes in the field: three facts, and the screen carries the
 * one that is true now. Null when nothing is running.
 */
export function runCopy(state: SignInState): { title: string; line: string } | null {
  switch (state.stage) {
    case 'Starting':
      return { title: 'Opening your browser', line: '' };
    case 'Waiting':
      return state.needs_code
        ? { title: 'Approve in your browser', line: 'Paste the code it gives you.' }
        : { title: 'Opening your browser', line: '' };
    case 'Finishing':
      return { title: 'Signing in', line: '' };
    default:
      return null;
  }
}
