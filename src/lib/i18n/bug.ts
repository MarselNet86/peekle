/**
 * The bug question the bug button raises. tech.md 6.22.
 */

import type { Copy } from './index.svelte';

const en = {
  title: 'Found a bug?',
  line: 'Tell us about it.',
  write: 'Write',
  cancel: 'Cancel',
};

export const BUG: Copy<typeof en> = {
  en,
  ru: {
    title: 'Нашли баг?',
    line: 'Сообщите нам.',
    write: 'Написать',
    cancel: 'Отмена',
  },
};
