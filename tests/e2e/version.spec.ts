/**
 * v87.2 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.5 and 9.
 *
 * Criteria, from the owner's request: the settings show the version of the
 * app. It stands as their last row, after the account, says the number Rust
 * answers with, and speaks the language of the rest of the island.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

async function stub(page: Page, language: 'en' | 'ru', signedIn: boolean) {
  await page.addInitScript(
    ({ language, signedIn }) => {
      const w = window as unknown as Record<string, unknown>;
      const handlers: Record<string, number> = {};
      let counter = 0;
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
      const account = {
        cli: 'Ready',
        version: '2.1.263',
        install: 'Homebrew',
        signed_in: signedIn,
        command: null,
      };

      const answer = (command: string): unknown => {
        switch (command) {
          case 'app_version':
            return '0.1.1';
          case 'get_language':
            return language;
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
          case 'get_models':
            return [];
          case 'get_account':
          case 'refresh_account':
            return account;
          case 'refresh_usage':
            return snapshot;
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
          return answer(command);
        },
      };
    },
    { language, signedIn },
  );
}

test.describe('the version row', () => {
  test('stands last in the settings, after the account', async ({ page }) => {
    await stub(page, 'en', true);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Settings' }).click();
    const rows = page.locator('.settings > *');
    await expect(rows.last()).toContainText('Version');
    await expect(rows.last()).toContainText('0.1.1');
    await expect(rows.nth((await rows.count()) - 2)).toContainText('Claude account');
  });

  test('speaks Russian with the rest of the island', async ({ page }) => {
    await stub(page, 'ru', true);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Настройки' }).click();
    const last = page.locator('.settings > *').last();
    await expect(last).toContainText('Версия');
    await expect(last).toContainText('0.1.1');
  });
});
