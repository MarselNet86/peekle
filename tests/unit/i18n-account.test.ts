/**
 * The sign-in window, the list, usage and the screenshot offer speak Russian
 * once Russian is chosen, and go back to English without a reload.
 * tech.md 6.28.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import { badgeHint } from '$lib/features/usage/badge.svelte';
import { notifyHint } from '$lib/features/notify/notify.svelte';
import { WINDOW_LABELS, connectLabel, reasonText } from '$lib/features/usage/usage.svelte';
import { i18n } from '$lib/i18n/index.svelte';
import { ageLabel } from '$lib/logic/age';
import { authCopy } from '$lib/logic/account';
import { reachCopy } from '$lib/logic/reach';
import { resetCountdown } from '$lib/logic/usage';
import ActionRow from '$lib/ui/ActionRow.svelte';
import AuthPanel from '$lib/ui/AuthPanel.svelte';
import CommandLine from '$lib/ui/CommandLine.svelte';
import RestMark from '$lib/ui/RestMark.svelte';
import SearchField from '$lib/ui/SearchField.svelte';
import SessionRow from '$lib/ui/SessionRow.svelte';
import ShotPrompt from '$lib/ui/ShotPrompt.svelte';
import UsageCorner from '$lib/ui/UsageCorner.svelte';
import type { AccountState } from '$lib/types/generated/AccountState';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { SignInState } from '$lib/types/generated/SignInState';
import type { UsageSnapshot } from '$lib/types/generated/UsageSnapshot';

afterEach(() => {
  i18n.set('en');
});

function account(patch: Partial<AccountState> = {}): AccountState {
  return {
    cli: 'Ready',
    version: '2.1.263',
    install: 'Native',
    signed_in: false,
    command: null,
    ...patch,
  } as AccountState;
}

function run(patch: Partial<SignInState> = {}): SignInState {
  return { stage: 'Idle', url: null, needs_code: false, error: null, ...patch };
}

function snapshot(reason: UsageSnapshot['reason']): UsageSnapshot {
  return {
    windows: [],
    source: 'Unavailable',
    reason,
    keychain_granted: false,
    fetched_at: 0,
    retry_after_ms: null,
  } as UsageSnapshot;
}

describe('the sign-in window in Russian', () => {
  it('words every screen', () => {
    i18n.set('ru');
    expect(authCopy('signin', account(), run()).title).toBe('Вход в Peekle');
    expect(authCopy('update', account({ version: '2.0.14' }), run()).line).toBe(
      'Claude Code 2.0.14 слишком старый для входа из Peekle.',
    );
    expect(authCopy('install', account(), run()).line).toContain('Терминале');
    expect(authCopy('failed', account(), run()).line).toBe('Claude Code не завершил вход.');
    // Rust's error is already in the language of the interface.
    expect(authCopy('failed', account(), run({ error: 'Неверный код' })).line).toBe('Неверный код');
  });

  it('draws the sign-in button and the install screen', () => {
    i18n.set('ru');
    const { unmount } = render(AuthPanel, { props: { account: account(), signIn: run() } });
    expect(screen.getByRole('heading', { name: 'Вход в Peekle' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Войти через Claude' })).toBeInTheDocument();
    unmount();

    render(AuthPanel, {
      props: {
        account: account({ cli: 'Missing', command: 'npm i -g @anthropic-ai/claude-code' }),
        signIn: run(),
      },
    });
    expect(screen.getByRole('button', { name: 'Проверить снова' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Скопировать' })).toBeInTheDocument();
    // The command is a command, never translated.
    expect(screen.getByText('npm i -g @anthropic-ai/claude-code')).toBeInTheDocument();
  });

  it('draws the waiting screen with its quiet ways out', async () => {
    i18n.set('ru');
    render(AuthPanel, {
      props: { account: account(), signIn: run({ stage: 'Waiting', url: 'https://claude.ai' }) },
    });
    expect(screen.getByRole('status', { name: 'Ждём ответа браузера' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Отмена' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Есть код?' }));
    expect(screen.getByPlaceholderText('Вставьте код')).toBeInTheDocument();
  });

  it('switches back to English without a remount', async () => {
    i18n.set('ru');
    render(CommandLine, { props: { command: 'claude update', copied: true } });
    expect(screen.getByRole('button', { name: 'Скопировано' })).toBeInTheDocument();
    i18n.set('en');
    await Promise.resolve();
    expect(await screen.findByRole('button', { name: 'Copied' })).toBeInTheDocument();
  });
});

describe('the connection and usage in Russian', () => {
  it('words the connection screen', () => {
    i18n.set('ru');
    expect(reachCopy('Offline')).toEqual({
      title: 'Нет подключения',
      line: 'Peekle не может связаться с Anthropic.',
      action: 'Повторить',
    });
  });

  it('words the reasons, the controls and the countdown', () => {
    i18n.set('ru');
    expect(reasonText(snapshot('NotGranted'))).toBe('нужен доступ к Связке ключей');
    expect(reasonText(null)).toBe('недоступно');
    expect(connectLabel(snapshot('Denied'))).toBe('Подключить');
    expect(WINDOW_LABELS.SevenDay).toBe('Неделя');
    expect(resetCountdown(5400, 0)).toBe('сброс через 1 ч 30 мин');
    expect(resetCountdown(30, 0)).toBe('сброс через <1 мин');
    expect(badgeHint()).toContain('10%');
    expect(notifyHint()).toContain('Системные настройки');
  });

  it('keeps the English wording byte for byte', () => {
    expect(WINDOW_LABELS.FiveHour).toBe('5h');
    expect(resetCountdown(300_000, 0)).toBe('resets in 3d 11h');
    expect(badgeHint()).toBe('The island shows it each time the 5h window crosses a ten.');
  });

  it('names the windows on the corner dials', () => {
    i18n.set('ru');
    render(UsageCorner, { props: { hour: 17, week: 48 } });
    expect(screen.getByText('17% 5 ч')).toBeInTheDocument();
    expect(screen.getByText('48% 7 д')).toBeInTheDocument();
  });
});

describe('the list in Russian', () => {
  const now = 10 * 24 * 3600_000;
  const card = {
    session: { project: 'peekle' },
    title: 'Fix the panel',
    status: 'Working',
    compacting: null,
    updated_at: now - 3 * 3600_000,
  } as unknown as SessionCard;

  it('words ages', () => {
    i18n.set('ru');
    expect(ageLabel(now - 30_000, now)).toBe('сейчас');
    expect(ageLabel(now - 5 * 60_000, now)).toBe('5 мин');
    expect(ageLabel(0, 500 * 365 * 24 * 3600_000)).toBe('99 г+');
  });

  it('draws a session row and asks before deleting', async () => {
    i18n.set('ru');
    render(SessionRow, { props: { card, now } });
    // The project is a name and stays as it is.
    expect(screen.getByText('peekle · работает')).toBeInTheDocument();
    expect(screen.getByText('3 ч')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Переименовать сессию' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Удалить чат' }));
    expect(screen.getByText('Удалить чат и его транскрипт?')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Удалить чат навсегда' })).toBeInTheDocument();
  });

  it('draws the search field and the resting mark', () => {
    i18n.set('ru');
    render(SearchField);
    expect(screen.getByPlaceholderText('Поиск сессий…')).toBeInTheDocument();
    render(RestMark, { props: { status: 'waiting', pct: 62, onopen: () => {} } });
    expect(
      screen.getByRole('button', {
        name: 'Claude ждёт вас, использовано 62% окна 5 ч. Открыть список сессий',
      }),
    ).toBeInTheDocument();
  });

  it('draws a settings action row with its cancel', async () => {
    i18n.set('ru');
    render(ActionRow, { props: { label: 'Claude', action: 'Выйти', confirm: 'Точно?' } });
    await userEvent.click(screen.getByRole('button', { name: 'Выйти' }));
    expect(screen.getByRole('button', { name: 'Отмена' })).toBeInTheDocument();
  });
});

describe('the screenshot offer in Russian', () => {
  it('words the pill and keeps the project name', () => {
    i18n.set('ru');
    render(ShotPrompt, { props: { project: 'peekle', secs: 4, onopen: () => {} } });
    expect(screen.getByText('Снимок для peekle')).toBeInTheDocument();
    expect(screen.getByText('Нажмите ⌘1, чтобы прикрепить')).toBeInTheDocument();
    expect(screen.getByText('4 с')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Открыть peekle' })).toBeInTheDocument();
  });
});
