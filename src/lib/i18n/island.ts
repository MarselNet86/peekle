/**
 * The words the island route says itself: the list, the settings, the chat
 * chrome around the primitives. tech.md 6.28.
 */

import type { Copy } from './index.svelte';

const en = {
  replyToClaude: 'Reply to Claude',
  sending: 'Sending…',
  messageClaude: 'Message Claude',
  backToList: 'Back to the session list',
  reportBug: 'Report a bug',
  reportBugHint: 'Tell the developer what broke. Opens Telegram.',
  settings: 'Settings',
  notifyTurn: 'Notify when a turn ends',
  showUsage: 'Show usage on the island',
  language: 'Language',
  account: 'Claude account',
  accountHint: 'Signed in to Claude Code on this Mac.',
  signOut: 'Sign out',
  signOutConfirm: 'Signs Claude Code out on this Mac, the terminal included.',
  newSession: 'New session',
  nothingMatches: 'Nothing matches that.',
  noSessions: 'No sessions yet. Run Claude Code once in a project and Peekle picks it up.',
  trustAsk: 'Claude Code asks whether you trust this folder',
  trustIt: 'Trust it',
  notHere: 'Not here',
  emptyContext: 'Nothing in the context yet',
  connecting: 'Connecting',
  connect: 'Connect',
  compacting: 'Compacting',
};

export const ISLAND: Copy<typeof en> = {
  en,
  ru: {
    replyToClaude: 'Ответить Claude',
    sending: 'Отправка…',
    messageClaude: 'Сообщение для Claude',
    backToList: 'К списку сессий',
    reportBug: 'Сообщить об ошибке',
    reportBugHint: 'Расскажите разработчику, что сломалось. Откроется Telegram.',
    settings: 'Настройки',
    notifyTurn: 'Уведомлять, когда Claude ответил',
    showUsage: 'Показывать лимиты на островке',
    language: 'Язык',
    account: 'Аккаунт Claude',
    accountHint: 'Вход в Claude Code на этом Mac выполнен.',
    signOut: 'Выйти',
    signOutConfirm: 'Claude Code выйдет из аккаунта на этом Mac, и в терминале тоже.',
    newSession: 'Новая сессия',
    nothingMatches: 'Ничего не найдено.',
    noSessions: 'Сессий пока нет. Запустите Claude Code в проекте, и Peekle её подхватит.',
    trustAsk: 'Claude Code спрашивает, доверяете ли вы этой папке',
    trustIt: 'Доверяю',
    notHere: 'Не здесь',
    emptyContext: 'Контекст пока пуст',
    connecting: 'Подключение',
    connect: 'Подключить',
    compacting: 'Сжатие',
  },
};
