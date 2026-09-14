/**
 * What the connection screen says. Pure, so the wording is a test rather than
 * a thing to read off a screenshot. tech.md 6.16.
 *
 * Only the network. Signing in is the sign-in window's (`logic/account.ts`):
 * until v83 this module also spoke for a login, off the reason in the usage
 * snapshot, and a refusal of the usage endpoint became a screen that told a
 * signed-in person to sign in.
 */

import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

export type ReachCopy = {
  title: string;
  line: string;
  /** What the button does, in the words of the thing it does. */
  action: string;
};

/**
 * Why the island cannot reach Anthropic, and the one thing that fixes it.
 * Null for every reason that does not stop an agent working. tech.md 6.16.
 */
export function reachCopy(reason: UsageUnavailable | null | undefined): ReachCopy | null {
  switch (reason) {
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
