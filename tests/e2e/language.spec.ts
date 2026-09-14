/**
 * v84 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.28.
 *
 * Criteria, from the owner's request: a first install asks for the language
 * first and only then for sign-in, on a screen with the title and two cards,
 * 🇷🇺 ru and 🇺🇸 en; the settings change the language through a list, as their
 * third row. Every command the route sends is kept on `window.__calls`.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

type Setup = { language: 'en' | 'ru' | null; signedIn: boolean };

async function stub(page: Page, setup: Setup) {
  await page.addInitScript(({ language, signedIn }) => {
    const w = window as unknown as Record<string, unknown>;
    const handlers: Record<string, number> = {};
    const calls: { command: string; args: unknown }[] = [];
    w.__calls = calls;
    let counter = 0;
    let chosen = language;
    const account = {
      cli: 'Ready',
      version: '2.1.263',
      install: 'Homebrew',
      signed_in: signedIn,
      command: null,
    };
    const snapshot = {
      windows: [
        { window: 'FiveHour', used_pct: 12, resets_at: null },
        { window: 'SevenDay', used_pct: 30, resets_at: null },
      ],
      source: 'Account',
      reason: null,
      fetched_at: 0,
      keychain_granted: true,
      retry_after_ms: null,
    };

    const answer = (command: string, args: Record<string, unknown> | undefined): unknown => {
      switch (command) {
        case 'get_language':
          return chosen;
        case 'set_language':
          chosen = args?.language as 'en' | 'ru';
          return null;
        case 'get_state':
          return {
            enabled: true,
            view: 'Sessions',
            active_prompt: null,
            sessions: [],
            tasks: [],
            usage: snapshot,
            shot: null,
            live_sessions: 0,
            hotkey_ok: true,
          };
        case 'get_sessions':
          return [];
        case 'get_account':
        case 'refresh_account':
          return account;
        case 'refresh_usage':
          return snapshot;
        case 'get_models':
          return [];
        default:
          return null;
      }
    };

    w.__TAURI_INTERNALS__ = {
      transformCallback: (cb: unknown) => {
        const id = ++counter;
        w[`_${id}`] = cb;
        return id;
      },
      convertFileSrc: (path: string) => path,
      invoke: async (command: string, args: Record<string, unknown>) => {
        if (command === 'plugin:event|listen') {
          handlers[args.event as string] = args.handler as number;
          return 1;
        }
        if (command.startsWith('plugin:event|')) return 1;
        calls.push({ command, args: args ?? null });
        return answer(command, args);
      },
    };
  }, setup);
}

async function calls(page: Page, command: string) {
  return page.evaluate(
    (name) =>
      (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls.filter(
        (call) => call.command === name,
      ),
    command,
  );
}

test.describe('the language screen', () => {
  test('a first run asks for the language before it asks for sign-in', async ({ page }) => {
    await stub(page, { language: null, signedIn: false });
    await page.goto(ROUTE);

    await expect(page.getByText('Выберите язык')).toBeVisible();
    await expect(page.getByText('Choose your language')).toBeVisible();
    const cards = page.getByRole('radio');
    await expect(cards).toHaveCount(2);
    await expect(cards.nth(0)).toContainText('🇷🇺');
    await expect(cards.nth(0)).toContainText('Русский');
    await expect(cards.nth(0)).toContainText('ru');
    await expect(cards.nth(1)).toContainText('🇺🇸');
    await expect(cards.nth(1)).toContainText('English');
    await expect(cards.nth(1)).toContainText('en');
    await expect(page.locator('[data-screen]')).toHaveCount(0);
  });

  test('picking Russian writes it and hands over to sign-in in Russian', async ({ page }) => {
    await stub(page, { language: null, signedIn: false });
    await page.goto(ROUTE);

    await page.getByRole('radio', { name: /Русский/ }).click();
    await expect(page.getByRole('radio', { name: /Русский/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(await calls(page, 'set_language')).toEqual([
      { command: 'set_language', args: { language: 'ru' } },
    ]);

    await expect(page.locator('[data-screen="signin"]')).toBeVisible();
    await expect(page.getByText('Выберите язык')).toHaveCount(0);
    await expect(page.locator('html')).toHaveAttribute('lang', 'ru');
    await expect(page.getByRole('button', { name: 'Sign in with Claude' })).toHaveCount(0);
  });

  test('picking English hands over to sign-in in English', async ({ page }) => {
    await stub(page, { language: null, signedIn: false });
    await page.goto(ROUTE);

    await page.getByRole('radio', { name: /English/ }).click();
    await expect(page.getByRole('button', { name: 'Sign in with Claude' })).toBeVisible();
    expect(await calls(page, 'set_language')).toEqual([
      { command: 'set_language', args: { language: 'en' } },
    ]);
  });

  test('a language already chosen never shows the screen', async ({ page }) => {
    await stub(page, { language: 'en', signedIn: false });
    await page.goto(ROUTE);

    await expect(page.locator('[data-screen="signin"]')).toBeVisible();
    await expect(page.getByText('Choose your language')).toHaveCount(0);
  });
});

test.describe('the language row', () => {
  test('stands third in the settings and switches the words', async ({ page }) => {
    await stub(page, { language: 'en', signedIn: true });
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Settings' }).click();
    const rows = page.locator('.settings > *');
    await expect(rows.nth(2)).toContainText('Language');
    await expect(rows.nth(2)).toContainText('English');

    await page.getByRole('button', { name: 'Language: English' }).click();
    await page.getByRole('option', { name: /Русский/ }).click();

    expect(await calls(page, 'set_language')).toEqual([
      { command: 'set_language', args: { language: 'ru' } },
    ]);
    await expect(rows.nth(2)).toContainText('Язык');
    await expect(rows.nth(2)).toContainText('Русский');
    await expect(page.getByRole('button', { name: 'Настройки' })).toBeVisible();
  });
});
