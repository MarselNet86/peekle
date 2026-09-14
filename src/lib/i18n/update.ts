/**
 * The update question. tech.md 6.30.
 */

import type { Copy } from './index.svelte';

const en = {
  /** The version rides in, so the title names what is being installed. */
  title: (version: string) => `Update to ${version}?`,
  /** The line under it for a bundle: the file is already here. */
  ready: (size: string) => `${size} downloaded and ready to install.`,
  /** The same line when the release named no size. */
  readyNoSize: 'Downloaded and ready to install.',
  /** And for a copy Homebrew owns, which installs it a different way. */
  brew: 'Homebrew installed this copy. Copy the command to upgrade.',
  install: 'Install',
  brewInstall: 'Copy command',
  later: 'Later',
  /** The title doubles as the link to the release page. */
  notes: 'Read the release notes',
};

export const UPDATE: Copy<typeof en> = {
  en,
  ru: {
    title: (version: string) => `Установить Peekle ${version}?`,
    ready: (size: string) => `${size} скачано, можно ставить.`,
    readyNoSize: 'Скачано, можно ставить.',
    brew: 'Эту копию поставил Homebrew. Скопируйте команду для обновления.',
    install: 'Установить',
    brewInstall: 'Скопировать',
    later: 'Позже',
    notes: 'Что нового',
  },
};
