/**
 * The language the island speaks: the first-run pick and the settings row.
 * Copies the island layout, bridge in and runes out, with Rust owning the
 * fact in the config. tech.md 6.28.
 */

import { commands } from '$lib/bridge';
import { i18n } from '$lib/i18n/index.svelte';
import { LANGUAGE_HOLD } from '$lib/logic/language';
import type { Language } from '$lib/types/generated/Language';

export function createLanguage() {
  let loaded = $state(false);
  let chosen = $state<Language | null>(null);
  // Pressed on the first-run screen and holding there for the tick.
  let picked = $state<Language | null>(null);
  let busy = $state(false);
  let holdTimer: ReturnType<typeof setTimeout> | undefined;

  async function start(): Promise<() => void> {
    try {
      const known = await commands.getLanguage();
      if (known) {
        chosen = known;
        i18n.set(known);
      }
    } catch {
      // Rust did not answer. Asking again is cheaper than guessing wrong, so
      // the screen stands and the next pick writes the config anyway.
    } finally {
      loaded = true;
    }
    return () => clearTimeout(holdTimer);
  }

  async function save(language: Language) {
    try {
      await commands.setLanguage(language);
    } catch {
      // Kept for this run all the same: the words already changed, and
      // flipping them back over a failed write helps nobody.
    }
  }

  /** The first-run pick. Every string switches at once; the screen holds the
   * picked card for `LANGUAGE_HOLD` and then gives way. tech.md 6.28. */
  async function pick(language: Language) {
    if (picked !== null) return;
    picked = language;
    i18n.set(language);
    await save(language);
    clearTimeout(holdTimer);
    holdTimer = setTimeout(() => {
      chosen = language;
      picked = null;
    }, LANGUAGE_HOLD);
  }

  /** The settings row. Nothing to hold: the row is already on screen. */
  async function change(language: Language) {
    if (busy || language === chosen) return;
    busy = true;
    chosen = language;
    i18n.set(language);
    try {
      await save(language);
    } finally {
      busy = false;
    }
  }

  return {
    get loaded() {
      return loaded;
    },
    get chosen() {
      return chosen;
    },
    get picked() {
      return picked;
    },
    get busy() {
      return busy;
    },
    start,
    pick,
    change,
  };
}
