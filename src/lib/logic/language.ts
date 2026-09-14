/**
 * The language screen and the language row, as rules. tech.md 6.28.
 */

import type { Language } from '$lib/types/generated/Language';

/** How long the picked card holds before the screen gives way. Long enough to
 * see the tick land, short enough that nobody waits for it. tech.md 6.28. */
export const LANGUAGE_HOLD = 420;

export type LanguageChoice = {
  id: Language;
  flag: string;
  /** In its own language: a person looks for the word they can read. */
  name: string;
};

/** Russian first, as the screen was laid out. tech.md 6.28. */
export const LANGUAGE_CHOICES: LanguageChoice[] = [
  { id: 'ru', flag: '🇷🇺', name: 'Русский' },
  { id: 'en', flag: '🇺🇸', name: 'English' },
];

/** The title is in both languages: nobody knows which one the person reads
 * until they have said. tech.md 6.28. */
export const LANGUAGE_TITLE = { ru: 'Выберите язык', en: 'Choose your language' };

/**
 * Whether the island has to ask for a language before anything else.
 *
 * Only on Rust's own word that none is chosen: before the answer arrives the
 * island does not know, and a screen that flashes up and away on every start
 * is worse than a moment of the usual one. Never over a waiting hook, for the
 * reason sign-in never is. tech.md 6.28.
 */
export function needsLanguage(
  loaded: boolean,
  chosen: Language | null,
  hasPrompt: boolean,
): boolean {
  return loaded && chosen === null && !hasPrompt;
}
