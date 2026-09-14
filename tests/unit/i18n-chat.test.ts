/**
 * The conversation speaks the language in force: the composer, the prompts,
 * the notes, the work line and the lines Rust writes about the chat.
 * tech.md 6.28.
 */

import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/index.svelte';
import {
  contextLabel,
  effortHint,
  ELSEWHERE_NOTE,
  MODE_NOTE,
  modeLabel,
  modeOptions,
  settingsNote,
} from '$lib/logic/agent';
import { splitPathNote } from '$lib/logic/files';
import { CHOOSE, folderOptions } from '$lib/logic/folders';
import { ASKED_TO_STOP, FORKED_NOTE, STOP_ASKED_NOTE } from '$lib/logic/sessions';
import { elapsedLabel, noticeText } from '$lib/logic/work';
import FeedRow from '$lib/ui/FeedRow.svelte';
import NoteBlock from '$lib/ui/NoteBlock.svelte';
import PermissionRow from '$lib/ui/PermissionRow.svelte';
import PromptInput from '$lib/ui/PromptInput.svelte';
import QuestionPrompt from '$lib/ui/QuestionPrompt.svelte';
import WorkLine from '$lib/ui/WorkLine.svelte';
import type { AgentSetup } from '$lib/types/generated/AgentSetup';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';

afterEach(() => i18n.set('en'));

function notice(text: string): FeedEntry {
  return { id: 'n1', kind: 'Notice', text, at: 0 } as unknown as FeedEntry;
}

describe('the lines Rust writes about the chat', () => {
  it('stay byte for byte in English', () => {
    for (const text of [
      'Compacted',
      'Compacted chat · manual · 466k tokens freed',
      'Asked Claude to stop',
      'Switched to Opus 4.1',
      'Thought for 12s',
      'anything the conversation said',
    ]) {
      expect(noticeText(text)).toBe(text);
    }
  });

  it('are said again in Russian', () => {
    i18n.set('ru');
    expect(noticeText('Compacted')).toBe('Чат сжат');
    expect(noticeText('Compacted chat · manual · 466k tokens freed')).toBe(
      'Чат сжат · вручную · освобождено токенов: 466k',
    );
    expect(noticeText('Compacted chat · auto')).toBe('Чат сжат · автоматически');
    expect(noticeText('Compacted chat · 812 tokens freed')).toBe(
      'Чат сжат · освобождено токенов: 812',
    );
    expect(noticeText('Asked Claude to stop')).toBe('Claude попросили остановиться');
    expect(noticeText('Switched to Opus 4.1')).toBe('Модель сменена на Opus 4.1');
    expect(noticeText('Thought for 12s')).toBe('Размышление: 12 с');
    expect(noticeText('Compacted chat, then more')).toBe('Compacted chat, then more');
  });

  it('render translated in the feed', () => {
    i18n.set('ru');
    render(FeedRow, { props: { entry: notice('Compacted chat · manual · 466k tokens freed') } });
    expect(screen.getByText('Чат сжат · вручную · освобождено токенов: 466k')).toBeInTheDocument();
  });
});

describe('the logic in Russian', () => {
  const opus = {
    context_pct: 61.2,
    context_tokens: 612_000,
    context_window: 1_000_000,
  } as unknown as AgentSetup;

  it('says the clock, the context and the hints', () => {
    i18n.set('ru');
    expect(elapsedLabel(9_400)).toBe('9 с');
    expect(elapsedLabel(64_000)).toBe('1 мин 4 с');
    expect(elapsedLabel(7_620_000)).toBe('2 ч 7 мин');
    expect(contextLabel(opus)).toBe(
      'Контекст заполнен на 61%: 612k из 1000k. Нажмите, чтобы сжать.',
    );
    expect(effortHint('Low')).toBe('Быстрая и простая реализация');
    expect(modeOptions()[0].hint).toBe('Спрашивает перед каждой правкой');
    expect(splitPathNote()).toMatch(/^Этот файл нельзя прикрепить/);
    expect(folderOptions([], '').find((row) => row.id === CHOOSE)?.label).toBe('Открыть папку…');
  });

  it('keeps the names the CLI gives its modes', () => {
    i18n.set('ru');
    expect(modeLabel('Plan')).toBe('Plan');
    expect(modeOptions()[0].label).toBe('Manual');
  });

  it('follows a switch in notes that are already standing', () => {
    const note = settingsNote({ origin: 'Observed', status: 'Idle' });
    expect(note).toBe(ELSEWHERE_NOTE);
    i18n.set('ru');
    expect(note?.fact).toMatch(/^Этот чат открыт в другом приложении/);
    expect(MODE_NOTE.fact).toBe('Режим выбирается при запуске сессии.');
    expect(FORKED_NOTE.fact).toMatch(/копия/);
    expect(STOP_ASKED_NOTE.fact).toBe('Peekle уже попросил этот чат остановиться.');
  });
});

describe('the components in Russian', () => {
  it('answer a permission with Разрешить and Запретить', () => {
    i18n.set('ru');
    const request = {
      id: 'p1',
      kind: 'Permission',
      title: 'Bash',
      detail: 'ls',
      options: [],
      questions: [],
      created_at: Date.now(),
    } as unknown as PromptRequest;
    render(PermissionRow, { props: { request } });
    expect(screen.getByRole('button', { name: 'Разрешить' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Запретить' })).toBeInTheDocument();
  });

  it('labels the send and stop buttons', () => {
    i18n.set('ru');
    const { unmount } = render(PromptInput, { props: { value: 'hi' } });
    expect(screen.getByRole('button', { name: 'Отправить' })).toBeInTheDocument();
    unmount();
    render(PromptInput, { props: { working: true } });
    expect(screen.getByRole('button', { name: 'Остановить' })).toBeInTheDocument();
  });

  it('offers a written answer to a question', () => {
    i18n.set('ru');
    render(QuestionPrompt, {
      props: {
        questions: [
          {
            header: 'Framework',
            question: 'Which one?',
            options: [{ label: 'Svelte', description: null }],
            multi_select: true,
          },
        ],
        onclose: () => {},
      },
    });
    expect(screen.getByText('Другое')).toBeInTheDocument();
    expect(screen.getByText('Напишите свой ответ')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Отправить' })).toBeInTheDocument();
    expect(screen.getByTitle('Закрыть вопрос')).toBeInTheDocument();
  });

  it('closes a note with Закрыть', () => {
    i18n.set('ru');
    render(NoteBlock, { props: { fact: MODE_NOTE.fact, how: MODE_NOTE.how, ttl: 0 } });
    expect(screen.getByText('Режим выбирается при запуске сессии.')).toBeInTheDocument();
    expect(screen.getByTitle('Закрыть')).toBeInTheDocument();
  });

  it('says a finished run and a stop request in Russian', async () => {
    i18n.set('ru');
    const { unmount } = render(WorkLine, { props: { running: false, from: 0, to: 64_000 } });
    expect(screen.getByText('Работа заняла 1 мин 4 с')).toBeInTheDocument();
    unmount();
    render(WorkLine, { props: { running: true, words: [ASKED_TO_STOP], from: Date.now() } });
    await screen.findByText('Запрошена остановка', {}, { timeout: 4000 });
  });
});
