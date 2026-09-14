/**
 * The copy of the conversation: the composer, the row under it, the menus it
 * opens, the notes that answer a press, and the permission and question
 * prompts. tech.md 6.28.
 *
 * Not here, on purpose: model names, the CLI's own names of the effort levels
 * and permission modes, key names and paths. Those are what Claude Code calls
 * them, and a translated name would be a name nobody can find in the CLI.
 */

import type { Effort } from '$lib/types/generated/Effort';
import type { TaskLabel } from '$lib/types/generated/TaskLabel';

import type { Copy } from './index.svelte';

type Note = { fact: string; how: string };

const en = {
  // The modes a menu offers, explained. The names stay the CLI's own.
  modeHints: {
    Manual: 'Asks before every edit',
    AcceptEdits: 'Edits go through, everything else asks',
    Plan: 'Reads and plans, changes nothing',
    Auto: 'Approves what passes its safety check',
  },
  effortHints: {
    Low: 'Quick, straightforward implementation',
    Medium: 'Balanced approach with standard testing',
    High: 'Comprehensive implementation with extensive testing',
    XHigh: 'Extended reasoning with thorough analysis',
    Max: 'Maximum capability with deepest reasoning',
  } as Record<Effort, string>,
  ultracodeHint: 'xhigh + dynamic workflows, this session only',
  contextUsed: (pct: number, used: number, window: number) =>
    `${pct}% of context used, ${used}k of ${window}k`,
  clickToCompact: 'Click to compact.',

  elsewhere: {
    fact: 'Another app is running this chat, so its model and effort are set there.',
    how: 'Close it there and your next message brings the chat here, controls and all.',
  } as Note,
  finished: 'This session has finished.',
  modeNote: {
    fact: 'The mode is chosen when a session starts.',
    how: 'Claude Code switches it with Shift+Tab in its own window.',
  } as Note,
  forked: {
    fact: 'Another app is holding that chat, so this is a copy of it.',
    how: 'Everything said so far came along. The original stays open where it was.',
  } as Note,
  stopAsked: {
    fact: 'Peekle has already asked this chat to stop.',
    how: 'It runs in another app, so the request waits until the agent reads it. Asking again would only queue a second message.',
  } as Note,

  model: 'Model',
  modelAndEffort: 'Model and effort',
  selectModel: 'Select a model',
  effort: 'Effort',
  thinking: 'Thinking',
  thinkingOff: 'off',
  thinkingHint: 'Set when the session starts',
  thinkingFixed: 'Thinking is set when a session starts',

  attachFiles: 'Attach files',
  send: 'Send',
  stop: 'Stop',

  claudeAsks: 'Claude asks',
  recommended: 'Recommended',
  other: 'Other',
  writeOwn: 'Write your own answer',
  closeQuestion: 'Close the question',
  typeAnswer: 'Type your answer…',
  submit: 'Submit',
  next: 'Next',

  openAsking: 'Open the session this is asking about',
  secsLeft: (secs: number) => `${secs}s`,
  allow: 'Allow',
  deny: 'Deny',
  dismiss: 'Dismiss',

  openFolder: 'Open folder…',
  splitPath: 'That file cannot be attached: its name runs onto a second line',

  taskLabels: {
    Code: 'Code',
    Fix: 'Fix',
    Bug: 'Bug',
    Test: 'Test',
    Docs: 'Docs',
    Chore: 'Chore',
    Research: 'Research',
  } as Record<TaskLabel, string>,
};

export const CHAT: Copy<typeof en> = {
  en,
  ru: {
    modeHints: {
      Manual: 'Спрашивает перед каждой правкой',
      AcceptEdits: 'Правки без вопросов, остальное с разрешения',
      Plan: 'Читает и планирует, ничего не меняет',
      Auto: 'Одобряет то, что проходит проверку безопасности',
    },
    effortHints: {
      Low: 'Быстрая и простая реализация',
      Medium: 'Сбалансированный подход со стандартными тестами',
      High: 'Полная реализация с подробными тестами',
      XHigh: 'Расширенное рассуждение и тщательный анализ',
      Max: 'Максимум возможностей и самое глубокое рассуждение',
    },
    ultracodeHint: 'xhigh и динамические сценарии, только эта сессия',
    contextUsed: (pct, used, window) => `Контекст заполнен на ${pct}%: ${used}k из ${window}k`,
    clickToCompact: 'Нажмите, чтобы сжать.',

    elsewhere: {
      fact: 'Этот чат открыт в другом приложении, поэтому модель и усилие задаются там.',
      how: 'Закройте его там, и следующее сообщение перенесёт чат сюда вместе с настройками.',
    },
    finished: 'Эта сессия завершена.',
    modeNote: {
      fact: 'Режим выбирается при запуске сессии.',
      how: 'Claude Code переключает его сочетанием Shift+Tab в своём окне.',
    },
    forked: {
      fact: 'Этот чат занят другим приложением, поэтому здесь его копия.',
      how: 'Вся переписка перенесена. Оригинал остался открытым там, где был.',
    },
    stopAsked: {
      fact: 'Peekle уже попросил этот чат остановиться.',
      how: 'Чат работает в другом приложении, и запрос ждёт, пока агент его прочтёт. Повторный запрос только поставит в очередь второе сообщение.',
    },

    model: 'Модель',
    modelAndEffort: 'Модель и усилие',
    selectModel: 'Выберите модель',
    effort: 'Усилие',
    thinking: 'Размышление',
    thinkingOff: 'выкл.',
    thinkingHint: 'Задаётся при запуске сессии',
    thinkingFixed: 'Размышление задаётся при запуске сессии',

    attachFiles: 'Прикрепить файлы',
    send: 'Отправить',
    stop: 'Остановить',

    claudeAsks: 'Claude спрашивает',
    recommended: 'Рекомендуется',
    other: 'Другое',
    writeOwn: 'Напишите свой ответ',
    closeQuestion: 'Закрыть вопрос',
    typeAnswer: 'Введите ответ…',
    submit: 'Отправить',
    next: 'Далее',

    openAsking: 'Открыть сессию, из которой пришёл запрос',
    secsLeft: (secs) => `${secs} с`,
    allow: 'Разрешить',
    deny: 'Запретить',
    dismiss: 'Закрыть',

    openFolder: 'Открыть папку…',
    splitPath: 'Этот файл нельзя прикрепить: его имя переносится на вторую строку',

    taskLabels: {
      Code: 'Код',
      Fix: 'Правка',
      Bug: 'Ошибка',
      Test: 'Тест',
      Docs: 'Доки',
      Chore: 'Рутина',
      Research: 'Анализ',
    },
  },
};
