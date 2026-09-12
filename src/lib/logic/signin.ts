/**
 * What the account screen says. Pure, so the wording is a test rather than a
 * thing to read off a screenshot. tech.md 6.16.
 */

import type { SignInFix } from '$lib/types/generated/SignInFix';
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
/**
 * Whether the panel offers a button at all.
 *
 * A refusal is the one state that does not: Claude Code is signed in, the
 * endpoint said no anyway, and every button this panel has would repeat
 * something that has already been tried. The poll clears the panel itself the
 * moment the endpoint answers. tech.md 6.16.
 */
export function canAct(state: SignInState): boolean {
  // A screen handing over a command keeps its button. The command is the
  // first half of the fix and the press is the second: somebody who has just
  // run `claude update` has to be able to say so, and a screen that ends in a
  // dead end sends them to relaunch the app to try again. Only a refusal
  // offers nothing, because there the press has already been made.
  // tech.md 6.16.
  return state.stage !== 'Refused';
}

/**
 * What the screen says when Claude Code itself is what is missing.
 *
 * Two sentences and a command, in that order: what is true, what to do, and
 * the line that does it. No link, no list of install methods, no apology --
 * the person came here to sign in, and the shortest way back to that is one
 * line they can paste. The words live here rather than in Rust, with the rest
 * of the copy; Rust says only which of the two it is. tech.md 6.16.
 */
export function fixCopy(fix: SignInFix): { title: string; line: string; label: string } {
  const label = `Run this in ${fix.shell}`;
  return fix.need === 'Update'
    ? {
        title: 'Claude Code is out of date',
        line: 'This version cannot sign in from the island. Update it, then press Sign in again.',
        label,
      }
    : {
        title: 'Claude Code is not installed',
        line: 'The island signs in through Claude Code. Install it, then press Sign in again.',
        label,
      };
}

export function runCopy(state: SignInState): { title: string; line: string } | null {
  switch (state.stage) {
    // Not a login that failed: a login that was never the answer. Claude Code
    // holds a credential and the endpoint refused it anyway, which is a
    // network or a region, not an account. tech.md 6.16.
    case 'Refused':
      return {
        title: 'Signed in, but the API refused',
        line: state.error ?? 'Check your connection or VPN, then wait a moment.',
      };
    case 'Starting':
      return { title: 'Opening your browser', line: '' };
    case 'Waiting':
      // No line under the title once the field is there: the placeholder
      // already says what goes in it, and saying it twice is one sentence
      // the reader has to skip.
      return state.needs_code
        ? { title: 'Approve in your browser', line: '' }
        : { title: 'Opening your browser', line: '' };
    case 'Finishing':
      return { title: 'Signing in', line: '' };
    default:
      return null;
  }
}
