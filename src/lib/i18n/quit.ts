/**
 * The quit question and the button that raises it. tech.md 6.29.
 */

import type { Copy } from './index.svelte';

const en = {
  title: 'Quit Peekle?',
  line: 'Are you sure you want to quit?',
  yes: 'Yes',
  no: 'No',
  button: 'Quit Peekle',
  hint: 'Asks first. ⌥⌘Q does the same from anywhere.',
};

export const QUIT: Copy<typeof en> = {
  en,
  ru: {
    title: 'Выйти из Peekle?',
    line: 'Вы уверены, что хотите выйти?',
    yes: 'Да',
    no: 'Нет',
    button: 'Выйти из Peekle',
    hint: 'Сначала спросит. То же делает ⌥⌘Q из любого места.',
  },
};
