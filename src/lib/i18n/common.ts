/** Words more than one primitive says. tech.md 6.28. */

import type { Copy } from './index.svelte';

const en = {
  cancel: 'Cancel',
  copy: 'Copy',
  copied: 'Copied',
  open: (name: string) => `Open ${name}`,
  remove: (name: string) => `Remove ${name}`,
  close: (name: string) => `Close ${name}`,
  emptyChat: 'Nothing said in this chat yet',
};

export const COMMON: Copy<typeof en> = {
  en,
  ru: {
    cancel: 'Отмена',
    copy: 'Скопировать',
    copied: 'Скопировано',
    open: (name) => `Открыть ${name}`,
    remove: (name) => `Убрать ${name}`,
    close: (name) => `Закрыть ${name}`,
    emptyChat: 'В этом чате пока ничего нет',
  },
};
