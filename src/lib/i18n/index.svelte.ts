/**
 * The language the island speaks, as one rune every string reads. tech.md 6.28.
 *
 * Copy is written once per area as a table of two halves of one type, so a
 * string missing from Russian is a type error rather than an English word on
 * a Russian screen. Reading `copy()` in markup or in a `$derived` tracks the
 * language, so a switch redraws every string without anyone listening for it.
 */

import type { Language } from '$lib/types/generated/Language';

/** Both halves of an area's copy, one shape between them. */
export type Copy<T> = Record<Language, T>;

// English until Rust says otherwise: it is the language the copy is written
// in, and a route rendered outside the shell never hears from Rust.
let current = $state<Language>('en');

export const i18n = {
  get language(): Language {
    return current;
  },
  /** Switches every string at once. */
  set(language: Language) {
    current = language;
    if (typeof document !== 'undefined') document.documentElement.lang = language;
  },
};

/** The half of a table in the language in force. */
export function copy<T>(table: Copy<T>): T {
  return table[current];
}
