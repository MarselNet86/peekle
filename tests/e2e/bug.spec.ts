/**
 * v87.4 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.22.
 *
 * Rust is played by the stub: `set_view` is answered with `peekle://view`,
 * and `open_bug_report` puts the island away the way Rust does once Telegram
 * is open. Every command the route sends is kept on `window.__calls`.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

async function stub(page: Page, telegramOpens = true) {
  await page.addInitScript((opens: boolean) => {
    const w = window as unknown as Record<string, unknown>;
    const handlers: Record<string, number> = {};
    const calls: { command: string; args: unknown }[] = [];
    w.__calls = calls;
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
      signed_in: true,
      command: null,
    };

    const push = (event: string, payload: unknown) => {
      const handler = w[`_${handlers[event]}`] as ((message: unknown) => void) | undefined;
      handler?.({ event, id: 1, payload });
    };

    const answer = (command: string, args: Record<string, unknown> | undefined): unknown => {
      switch (command) {
        case 'get_language':
          return 'en';
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
        case 'set_view':
          setTimeout(() => push('peekle://view', args?.view), 0);
          return null;
        case 'open_bug_report':
          if (!opens) throw 'Could not open Telegram';
          // Rust puts the island away once Telegram is open. tech.md 6.22.
          setTimeout(() => push('peekle://view', 'Collapsed'), 0);
          return null;
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
    w.__view = (view: unknown) => push('peekle://view', view);
    w.__listening = (event: string) => event in handlers;
  }, telegramOpens);
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

async function view(page: Page, next: unknown) {
  await page.waitForFunction(() =>
    (window as unknown as { __listening: (event: string) => boolean }).__listening('peekle://view'),
  );
  await page.evaluate(
    (value) => (window as unknown as { __view: (v: unknown) => void }).__view(value),
    next,
  );
}

test.describe('reporting a bug', () => {
  test('the bug button folds the list into the question', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Sessions');

    await page.getByRole('button', { name: 'Report a bug' }).click();
    await expect(page.getByRole('alertdialog')).toBeVisible();
    await expect(page.getByText('Found a bug?')).toBeVisible();
    await expect(page.getByText('Tell us about it.')).toBeVisible();
    await expect(page.getByRole('button', { name: 'New session' })).toHaveCount(0);

    const setView = await calls(page, 'set_view');
    expect(setView.at(-1)).toEqual({ command: 'set_view', args: { view: 'Bug' } });
    expect(await calls(page, 'open_bug_report')).toHaveLength(0);

    await expect
      .poll(async () => (await page.locator('.shape').boundingBox())?.height ?? 999)
      .toBeLessThan(120);
  });

  test('Write opens Telegram once and the island closes completely', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Sessions');

    await page.getByRole('button', { name: 'Report a bug' }).click();
    await page.getByRole('button', { name: 'Write' }).click();

    const sent = await calls(page, 'open_bug_report');
    expect(sent).toHaveLength(1);
    // The address is Rust's, so the press carries no address of its own.
    expect(Object.keys((sent[0].args ?? {}) as Record<string, unknown>)).toEqual([]);
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    await expect
      .poll(async () => (await page.locator('.shape').boundingBox())?.height ?? 0)
      .toBeLessThan(40);
  });

  test('Cancel goes back to the list the question was asked over', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Sessions');

    await page.getByRole('button', { name: 'Report a bug' }).click();
    await page.getByRole('button', { name: 'Cancel' }).click();

    const setView = await calls(page, 'set_view');
    expect(setView.at(-1)).toEqual({ command: 'set_view', args: { view: 'Sessions' } });
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    expect(await calls(page, 'open_bug_report')).toHaveLength(0);
  });

  test('a Telegram that would not open says so and keeps the question up', async ({ page }) => {
    await stub(page, false);
    await page.goto(ROUTE);
    await view(page, 'Sessions');

    await page.getByRole('button', { name: 'Report a bug' }).click();
    await page.getByRole('button', { name: 'Write' }).click();

    await expect(page.getByText('Could not open Telegram')).toBeVisible();
    await expect(page.getByRole('alertdialog')).toBeVisible();
  });
});
