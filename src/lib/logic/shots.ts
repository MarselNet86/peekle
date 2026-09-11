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
 * Whether a paste is a picture rather than text, judged before anything reads
 * the clipboard's contents. tech.md 6.13.
 *
 * The webview pastes text itself and must be left to it: intercepting every
 * paste to ask Rust would put a system paste prompt in front of somebody
 * pasting a word. So text wins whenever it is there, and Rust is asked only
 * when the types say picture and nothing says text. Reading the types raises
 * nothing; reading the contents is what does.
 */
export function looksLikeImagePaste(types: readonly string[]): boolean {
  if (types.some((type) => type === 'text/plain' || type === 'text/html')) return false;
  return types.some((type) => type.startsWith('image/') || type === 'Files');
}

/**
 * A line of a reply that is an attached screenshot, and not prose that
 * happens to mention one. tech.md 6.13.
 *
 * By shape and never by reading the disk: the whole line, the shots
 * directory, a ulid, `.png`. A path Peekle wrote is the only thing that
 * matches, so a person quoting a filename keeps their words.
 */
const SHOT_LINE = /^\/.*\/peekle\/shots\/[0-9A-HJKMNP-TV-Z]{26}\.png$/;

/**
 * Whether this path is one Peekle wrote for a screenshot.
 *
 * By shape, never by reading the disk, and used by more than the split below:
 * the chip above the field asks it to know whether it has a picture to show,
 * and the file rule (6.25) asks it to keep its hands off a screenshot.
 */
export function isShot(path: string): boolean {
  return SHOT_LINE.test(path.trim());
}

/** A reply split into the shots it carries and the words that go with them. */
export interface SaidWithShots {
  shots: string[];
  said: string;
}

/**
 * What a reply says and what it carries. `compose` puts each path on its own
 * line before the text, so the split is the same one in reverse. tech.md 6.13.
 */
export function shotLines(text: string): SaidWithShots {
  const shots: string[] = [];
  const words: string[] = [];

  for (const line of text.split('\n')) {
    if (isShot(line)) {
      shots.push(line.trim());
      continue;
    }
    words.push(line);
  }

  return { shots, said: words.join('\n').trim() };
}

/**
 * How big the picture is, in the words the CLI uses for the same thing:
 * `1172×246`. tech.md 6.13.
 *
 * `null` until the picture has been measured, which is what a browser answers
 * with before it has loaded one: a size of `0×0` on screen is worse than no
 * size at all. Read off the image itself rather than carried on the entry --
 * the webview has already loaded the file to draw it, and a number Rust sent
 * would be a second answer to a question the picture answers.
 */
export function shotSize(width: number, height: number): string | null {
  if (!Number.isFinite(width) || !Number.isFinite(height)) return null;
  if (width <= 0 || height <= 0) return null;
  return `${Math.round(width)}\u00d7${Math.round(height)}`;
}

/**
 * What the chip above the field calls the attachment. The full path is what
 * the agent gets, and it is far too long to sit over a reply box.
 */
export function shotName(path: string): string {
  const name = path.split('/').filter(Boolean).pop();
  return name ?? path;
}
