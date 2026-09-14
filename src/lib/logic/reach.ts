/**
 * What the connection screen says. Pure, so the wording is a test rather than
 * a thing to read off a screenshot. tech.md 6.16.
 *
 * Only the network. Signing in is the sign-in window's (`logic/account.ts`):
 * until v83 this module also spoke for a login, off the reason in the usage
 * snapshot, and a refusal of the usage endpoint became a screen that told a
 * signed-in person to sign in.
 */

import { ACCOUNT } from '$lib/i18n/account';
import { copy } from '$lib/i18n/index.svelte';
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
  const t = copy(ACCOUNT);
  switch (reason) {
    case 'Offline':
      return { title: t.offlineTitle, line: t.offlineLine, action: t.tryAgain };
    case 'Network':
      return { title: t.networkTitle, line: t.networkLine, action: t.tryAgain };
    default:
      return null;
  }
}
