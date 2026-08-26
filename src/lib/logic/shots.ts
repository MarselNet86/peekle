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
 * Whole seconds left on the offer, rounded up, and never below zero.
 *
 * Up rather than down: with 4.2 seconds left the honest answer to "have I got
 * time to reach the key" is five, and a `0s` standing on screen while the key
 * still works reads as a broken offer. Zero is never shown, because at zero
 * there is no offer left to show it on. tech.md 6.13.
 */
export function secondsLeft(offer: ShotOffer, now: number): number {
  const left = offer.expires_at - now;
  if (!Number.isFinite(left) || left <= 0) return 0;
  return Math.ceil(left / 1000);
}

/**
 * What the chip above the field calls the attachment. The full path is what
 * the agent gets, and it is far too long to sit over a reply box.
 */
export function shotName(path: string): string {
  const name = path.split('/').filter(Boolean).pop();
  return name ?? path;
}
