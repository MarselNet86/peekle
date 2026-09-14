/**
 * The copy of the feed: the work line, its clock, the fold of a message, and
 * the lines Rust writes about the conversation. tech.md 6.28.
 *
 * Rust sends those lines in English (`transcripts::COMPACTED`,
 * `compact_label`, `ASKED_TO_STOP`, `Switched to`, `Thought for`), and they are
 * matched by that exact shape and said again here. The conversation itself is
 * never translated.
 */

import type { Copy } from './index.svelte';

const en = {
  /** The words the work line types while a turn runs. */
  workWords: ['Working', 'Reading', 'Thinking', 'Writing', 'Checking'],
  /** Words a caller hands the line in English, said in the language in force. */
  words: {
    'Asked to stop': 'Asked to stop',
    Compacting: 'Compacting',
  } as Record<string, string>,
  worked: 'Worked',
  workedFor: (clock: string) => `Worked for ${clock}`,

  secs: (s: number) => `${s}s`,
  minsSecs: (m: number, s: number) => `${m}m ${s}s`,
  hoursMins: (h: number, m: number) => `${h}h ${m}m`,

  foldMessage: 'Fold this message',
  openMessage: 'Open this message',
  scrollNewest: 'Scroll to the newest message',

  compacted: 'Compacted',
  compactedChat: 'Compacted chat',
  triggers: { manual: 'manual', auto: 'auto' } as Record<string, string>,
  tokensFreed: (count: string) => `${count} tokens freed`,
  askedClaudeToStop: 'Asked Claude to stop',
  switchedTo: (name: string) => `Switched to ${name}`,
  thoughtFor: (clock: string) => `Thought for ${clock}`,
};

export const FEED: Copy<typeof en> = {
  en,
  ru: {
    workWords: ['Работает', 'Читает', 'Думает', 'Пишет', 'Проверяет'],
    words: {
      'Asked to stop': 'Запрошена остановка',
      Compacting: 'Сжимает чат',
    },
    worked: 'Работа завершена',
    workedFor: (clock) => `Работа заняла ${clock}`,

    secs: (s) => `${s} с`,
    minsSecs: (m, s) => `${m} мин ${s} с`,
    hoursMins: (h, m) => `${h} ч ${m} мин`,

    foldMessage: 'Свернуть сообщение',
    openMessage: 'Развернуть сообщение',
    scrollNewest: 'Прокрутить к последнему сообщению',

    compacted: 'Чат сжат',
    compactedChat: 'Чат сжат',
    triggers: { manual: 'вручную', auto: 'автоматически' },
    tokensFreed: (count) => `освобождено токенов: ${count}`,
    askedClaudeToStop: 'Claude попросили остановиться',
    switchedTo: (name) => `Модель сменена на ${name}`,
    thoughtFor: (clock) => `Размышление: ${clock}`,
  },
};
