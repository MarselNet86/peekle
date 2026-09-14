/** The sign-in window, the connection screen and the notify switch. tech.md 6.16, 6.17, 6.28. */

import type { Copy } from './index.svelte';

const en = {
  installTitle: 'Install Claude Code',
  installLine: 'Peekle runs on top of Claude Code. Install it in Terminal, then come back.',
  updateTitle: 'Update Claude Code',
  tooOld: (version: string) => `Claude Code ${version} is too old to sign in from Peekle.`,
  tooOldUnknown: 'This version of Claude Code is too old to sign in from Peekle.',
  signinTitle: 'Sign in to Peekle',
  signinLine: 'Peekle uses your Claude account through Claude Code.',
  startingTitle: 'Opening your browser',
  waitingTitle: 'Continue in your browser',
  waitingLine: 'Approve access on claude.ai. Peekle picks it up on its own.',
  finishingTitle: 'Signing in',
  doneTitle: "You're signed in",
  deniedTitle: 'Access declined',
  deniedLine: "Nothing was shared with Peekle. Try again whenever you're ready.",
  failedTitle: "Sign-in didn't finish",
  failedLine: 'Claude Code did not finish signing in.',

  checkAgain: 'Check again',
  installGuide: 'Installation guide',
  whatsNew: "What's new",
  updateGuide: 'Update guide',
  signIn: 'Sign in with Claude',
  waitingBrowser: 'Waiting for your browser',
  pasteCode: 'Paste the code',
  continue: 'Continue',
  openAgain: 'Open the page again',
  haveCode: 'Have a code?',
  tryAgain: 'Try again',

  offlineTitle: 'No connection',
  offlineLine: 'Peekle cannot reach Anthropic.',
  networkTitle: 'No answer',
  networkLine: 'Anthropic did not respond.',

  notifyHint: 'macOS decides whether banners appear. Check System Settings › Notifications.',
};

export const ACCOUNT: Copy<typeof en> = {
  en,
  ru: {
    installTitle: 'Установите Claude Code',
    installLine: 'Peekle работает поверх Claude Code. Установите его в Терминале и возвращайтесь.',
    updateTitle: 'Обновите Claude Code',
    tooOld: (version) => `Claude Code ${version} слишком старый для входа из Peekle.`,
    tooOldUnknown: 'Эта версия Claude Code слишком старая для входа из Peekle.',
    signinTitle: 'Вход в Peekle',
    signinLine: 'Peekle работает с вашим аккаунтом Claude через Claude Code.',
    startingTitle: 'Открываем браузер',
    waitingTitle: 'Продолжите в браузере',
    waitingLine: 'Разрешите доступ на claude.ai. Peekle заметит это сам.',
    finishingTitle: 'Выполняется вход',
    doneTitle: 'Вход выполнен',
    deniedTitle: 'Доступ отклонён',
    deniedLine: 'Peekle ничего не получил. Повторите, когда будете готовы.',
    failedTitle: 'Вход не завершён',
    failedLine: 'Claude Code не завершил вход.',

    checkAgain: 'Проверить снова',
    installGuide: 'Как установить',
    whatsNew: 'Что нового',
    updateGuide: 'Как обновить',
    signIn: 'Войти через Claude',
    waitingBrowser: 'Ждём ответа браузера',
    pasteCode: 'Вставьте код',
    continue: 'Продолжить',
    openAgain: 'Открыть страницу снова',
    haveCode: 'Есть код?',
    tryAgain: 'Повторить',

    offlineTitle: 'Нет подключения',
    offlineLine: 'Peekle не может связаться с Anthropic.',
    networkTitle: 'Нет ответа',
    networkLine: 'Anthropic не отвечает.',

    notifyHint: 'Показ баннеров решает macOS. Проверьте Системные настройки › Уведомления.',
  },
};
