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

/**
 * How long the compact panel stands. The same twenty seconds Rust holds the
 * island open for; it is the panel's clock, never the hook's. tech.md 6.7.
 */
export const ASK_SECS = 20;

/** Seconds left of those twenty, never below zero and never above the whole. */
export function secsLeft(createdAt: number, now: number): number {
  if (!Number.isFinite(createdAt) || !Number.isFinite(now)) return ASK_SECS;
  const gone = Math.floor((now - createdAt) / 1000);
  return Math.min(ASK_SECS, Math.max(0, ASK_SECS - gone));
}

/** A permission is the only prompt that must never be answered by accident. */
export function isPermission(request: PromptRequest | null): boolean {
  return request?.kind === 'Permission';
}

/** AskUserQuestion: its own questions, not a flat allow/deny list. tech.md 6.14. */
export function isQuestion(request: PromptRequest | null): boolean {
  return request?.kind === 'Question';
}
