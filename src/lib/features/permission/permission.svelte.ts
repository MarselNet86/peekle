/**
 * Permission state. Thin on purpose: the request itself lives in the island
 * shell alongside every other blocking prompt, and this only names which of
 * its choices Allow and Deny map to.
 *
 * The mapping is 6.2. Allow answers with `allow_once`; `allow_always` is
 * defined there too but S4 puts two buttons on screen, not three.
 */

import type { PromptRequest } from '$lib/types/generated/PromptRequest';

export function choiceFor(
  request: PromptRequest | null,
  kind: 'allow' | 'deny',
): string | undefined {
  const wanted = kind === 'allow' ? 'AllowOnce' : 'Deny';
  return request?.options.find((option) => option.kind === wanted)?.id;
}

/** A permission is the only prompt that must never be answered by accident. */
export function isPermission(request: PromptRequest | null): boolean {
  return request?.kind === 'Permission';
}

/** AskUserQuestion: its own questions, not a flat allow/deny list. tech.md 6.14. */
export function isQuestion(request: PromptRequest | null): boolean {
  return request?.kind === 'Question';
}
