/** The usage bars, dials, badge and the reasons they are empty. tech.md 6.4, 6.18, 6.28. */

import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';
import type { UsageWindow } from '$lib/types/generated/UsageWindow';
import type { Copy } from './index.svelte';

const en = {
  reasons: {
    Disabled: 'usage is off in the config',
    NotGranted: 'needs Keychain access',
    Denied: 'Keychain access was denied',
    NotLoggedIn: 'not signed in',
    Offline: 'no internet connection',
    Network: 'could not reach the API',
    RateLimited: 'too many requests, it asked to wait',
    Unsupported: 'the API stopped reporting it',
  } as Record<UsageUnavailable, string>,
  unavailable: 'unavailable',
  windows: { FiveHour: '5h', SevenDay: 'Week' } as Record<UsageWindow, string>,
  connect: 'Connect',
  reconnect: 'Reconnect',

  resetsIn: (span: string) => `resets in ${span}`,
  underMinute: '<1m',
  minutes: (n: number) => `${n}m`,
  hours: (n: number) => `${n}h`,
  days: (n: number) => `${n}d`,
  /** Two units, the larger first: `1h 30m`. */
  pair: (big: string, small: string) => `${big} ${small}`,

  /** The short names of the two windows on a dial. */
  hour: '5h',
  week: '7d',
  dialTitle: (pct: number, label: string | undefined) => `${pct}% of ${label ?? 'the window'}`,

  badgeHint: 'The island shows it each time the 5h window crosses a ten.',
};

export const USAGE: Copy<typeof en> = {
  en,
  ru: {
    reasons: {
      Disabled: 'учёт расхода выключен в конфиге',
      NotGranted: 'нужен доступ к Связке ключей',
      Denied: 'доступ к Связке ключей запрещён',
      NotLoggedIn: 'вход не выполнен',
      Offline: 'нет подключения к интернету',
      Network: 'API недоступен',
      RateLimited: 'слишком много запросов, API просит подождать',
      Unsupported: 'API больше не сообщает расход',
    },
    unavailable: 'недоступно',
    windows: { FiveHour: '5 ч', SevenDay: 'Неделя' },
    connect: 'Подключить',
    reconnect: 'Подключить снова',

    resetsIn: (span) => `сброс через ${span}`,
    underMinute: '<1 мин',
    minutes: (n) => `${n} мин`,
    hours: (n) => `${n} ч`,
    days: (n) => `${n} д`,
    pair: (big, small) => `${big} ${small}`,

    hour: '5 ч',
    week: '7 д',
    dialTitle: (pct, label) => (label ? `${pct}%, ${label}` : `${pct}% окна`),

    badgeHint: 'Островок показывает процент каждый раз, когда окно 5 ч проходит очередные 10%.',
  },
};
