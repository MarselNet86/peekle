/**
 * v84 acceptance: the language of the interface. tech.md 6.28.
 *
 * Criteria, from the owner's request: a first install asks for the language
 * before it asks for sign-in, on a screen with the title and two cards, 🇷🇺
 * Russian and 🇺🇸 English; the settings change the language through a list,
 * as their third row; and choosing a language changes the words.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { commands } from '$lib/bridge';
import { createLanguage } from '$lib/features/language/language.svelte';
import { copy, i18n } from '$lib/i18n/index.svelte';
import { LANGUAGE_CHOICES, LANGUAGE_HOLD, needsLanguage } from '$lib/logic/language';
import LanguagePicker from '$lib/ui/LanguagePicker.svelte';
import SelectRow from '$lib/ui/SelectRow.svelte';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: { ...real.commands, getLanguage: vi.fn(), setLanguage: vi.fn() },
  };
});

beforeEach(() => {
  vi.mocked(commands.getLanguage).mockReset().mockResolvedValue(null);
  vi.mocked(commands.setLanguage).mockReset().mockResolvedValue(null);
});

afterEach(() => {
  vi.useRealTimers();
  i18n.set('en');
});

describe('the language gate', () => {
  it('asks only once Rust has said nothing is chosen', () => {
    expect(needsLanguage(false, null, false)).toBe(false);
    expect(needsLanguage(true, null, false)).toBe(true);
    expect(needsLanguage(true, 'ru', false)).toBe(false);
  });

  it('never stands in front of a waiting hook', () => {
    expect(needsLanguage(true, null, true)).toBe(false);
  });
});

describe('the language screen', () => {
  it('offers Russian and English, each with its flag and code', () => {
    expect(LANGUAGE_CHOICES.map((choice) => [choice.flag, choice.name, choice.id])).toEqual([
      ['🇷🇺', 'Русский', 'ru'],
      ['🇺🇸', 'English', 'en'],
    ]);

    render(LanguagePicker);
    expect(screen.getByText('Выберите язык')).toBeInTheDocument();
    expect(screen.getByText('Choose your language')).toBeInTheDocument();
    const cards = screen.getAllByRole('radio');
    expect(cards).toHaveLength(2);
    expect(cards[0]).toHaveTextContent(/🇷🇺\s*Русский\s*ru/);
    expect(cards[1]).toHaveTextContent(/🇺🇸\s*English\s*en/);
  });

  it('picks on the press, with no button to confirm', async () => {
    const onpick = vi.fn();
    render(LanguagePicker, { props: { onpick } });
    await userEvent.click(screen.getByRole('radio', { name: /Русский/ }));
    expect(onpick).toHaveBeenCalledWith('ru');
  });

  it('marks the picked card and takes no second pick', () => {
    render(LanguagePicker, { props: { value: 'en' } });
    expect(screen.getByRole('radio', { name: /English/ })).toHaveAttribute('aria-checked', 'true');
    for (const card of screen.getAllByRole('radio')) expect(card).toBeDisabled();
  });
});

describe('choosing a language', () => {
  it('keeps a language already in the config and skips the screen', async () => {
    vi.mocked(commands.getLanguage).mockResolvedValue('ru');
    const language = createLanguage();
    await language.start();
    expect(language.chosen).toBe('ru');
    expect(i18n.language).toBe('ru');
  });

  it('switches the words at once and gives way after the hold', async () => {
    vi.useFakeTimers();
    const table = { en: 'Settings', ru: 'Настройки' };
    const language = createLanguage();
    await language.start();
    expect(language.chosen).toBeNull();

    await language.pick('ru');
    expect(copy(table)).toBe('Настройки');
    expect(commands.setLanguage).toHaveBeenCalledWith('ru');
    expect(language.picked).toBe('ru');
    expect(needsLanguage(language.loaded, language.chosen, false)).toBe(true);

    vi.advanceTimersByTime(LANGUAGE_HOLD);
    expect(language.chosen).toBe('ru');
    expect(needsLanguage(language.loaded, language.chosen, false)).toBe(false);
  });

  it('changes from the settings without a screen', async () => {
    vi.mocked(commands.getLanguage).mockResolvedValue('en');
    const language = createLanguage();
    await language.start();
    await language.change('ru');
    expect(language.chosen).toBe('ru');
    expect(i18n.language).toBe('ru');
    expect(commands.setLanguage).toHaveBeenCalledWith('ru');
  });
});

describe('the language row', () => {
  const options = LANGUAGE_CHOICES.map((choice) => ({
    id: choice.id,
    label: choice.name,
    icon: choice.flag,
  }));

  it('shows the language in force and opens a list of both', async () => {
    const onchange = vi.fn();
    render(SelectRow, { props: { label: 'Language', options, value: 'en', onchange } });

    await userEvent.click(screen.getByRole('button', { name: 'Language: English' }));
    const list = screen.getByRole('listbox');
    expect(list).toBeInTheDocument();
    expect(screen.getByRole('option', { name: /English/ })).toHaveAttribute(
      'aria-selected',
      'true',
    );

    await userEvent.click(screen.getByRole('option', { name: /Русский/ }));
    expect(onchange).toHaveBeenCalledWith('ru');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('closes on Escape without changing anything', async () => {
    const onchange = vi.fn();
    render(SelectRow, { props: { label: 'Language', options, value: 'en', onchange } });
    await userEvent.click(screen.getByRole('button', { name: 'Language: English' }));
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(onchange).not.toHaveBeenCalled();
  });
});
