/**
 * Pure logic behind the screenshot offer. tech.md 6.13.
 */

import type { ShotOffer } from '$lib/types/generated/ShotOffer';

/**
 * How much of the offer is left, from 1 at the moment it went up to 0 when it
 * is over. Total over any clock: an offer already gone reads as 0 rather than
 * as a negative bar, and one from the future reads as full.
 */
export function timeLeft(offer: ShotOffer, now: number): number {
  const span = offer.expires_at - offer.created_at;
  if (!Number.isFinite(span) || span <= 0) return 0;
  const left = (offer.expires_at - now) / span;
  if (Number.isNaN(left)) return 0;
  return Math.min(1, Math.max(0, left));
}

/**
 * What the chip above the field calls the attachment. The full path is what
 * the agent gets, and it is far too long to sit over a reply box.
 */
export function shotName(path: string): string {
  const name = path.split('/').filter(Boolean).pop();
  return name ?? path;
}
