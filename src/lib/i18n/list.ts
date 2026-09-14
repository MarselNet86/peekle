/** The session list and the resting mark. tech.md 6.7, 6.26, 6.28. */

import type { RestStatus } from '$lib/logic/rest';
import type { Copy } from './index.svelte';

const en = {
  search: 'Search sessions…',
  searchLabel: 'Search sessions',
  rename: 'Rename this session',
  delete: 'Delete this chat',
  deleteForGood: 'Delete this chat for good',
  deleteAsk: 'Delete this chat and its transcript?',
  status: {
    Working: 'working',
    WaitingOnUser: 'waiting on you',
    Idle: 'idle',
    Ended: 'ended',
  } as Record<string, string>,

  /** Ages as a picker writes them: one unit, no space. */
  now: 'now',
  minutes: (n: number) => `${n}m`,
  hours: (n: number) => `${n}h`,
  days: (n: number) => `${n}d`,
  years: (n: number, more: boolean) => `${n}y${more ? '+' : ''}`,

  rest: {
    idle: 'Peekle is running',
    working: 'Claude is working',
    waiting: 'Claude is waiting on you',
    compacting: 'Claude is compacting the chat',
  } as Record<RestStatus, string>,
  restUsed: (pct: number) => `, ${pct}% of the 5h window used`,
  restOpen: (said: string) => `${said}. Open the session list`,
};

export const LIST: Copy<typeof en> = {
  en,
  ru: {
    search: 'Поиск сессий…',
    searchLabel: 'Поиск сессий',
    rename: 'Переименовать сессию',
    delete: 'Удалить чат',
    deleteForGood: 'Удалить чат навсегда',
    deleteAsk: 'Удалить чат и его транскрипт?',
    status: {
      Working: 'работает',
      WaitingOnUser: 'ждёт вас',
      Idle: 'простаивает',
      Ended: 'завершена',
    },

    now: 'сейчас',
    minutes: (n) => `${n} мин`,
    hours: (n) => `${n} ч`,
    days: (n) => `${n} д`,
    years: (n, more) => `${n} г${more ? '+' : ''}`,

    rest: {
      idle: 'Peekle работает',
      working: 'Claude работает',
      waiting: 'Claude ждёт вас',
      compacting: 'Claude сжимает чат',
    },
    restUsed: (pct) => `, использовано ${pct}% окна 5 ч`,
    restOpen: (said) => `${said}. Открыть список сессий`,
  },
};
