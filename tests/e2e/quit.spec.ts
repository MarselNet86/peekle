/**
 * v85 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.29.
 *
 * Rust is played by the stub: `set_view` is answered the way Rust answers it,
 * with `peekle://view`, and ⌥⌘Q is played by pushing `Quit` the way the
 * shortcut handler does. Every command the route sends is kept on
 * `window.__calls`.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

async function stub(page: Page) {
  await page.addInitScript(() => {
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
          return [];
        case 'get_account':
        case 'refresh_account':
          return account;
        case 'refresh_usage':
          return snapshot;
        case 'get_models':
          return [];
        case 'set_view':
          // Rust stores the intent and says so on the event.
          setTimeout(() => push('peekle://view', args?.view), 0);
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
    // Whether the route has subscribed yet: an event sent before that reaches
    // nobody, the same as it would from Rust. tech.md section 8.
    w.__listening = (event: string) => event in handlers;
  });
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

test.describe('quitting', () => {
  test('the button after the gear folds the list into the question', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Sessions');

    const corner = page.locator('.corner button');
    await expect(corner.last()).toHaveAttribute('aria-label', 'Quit Peekle');
    await expect(corner.nth(1)).toHaveAttribute('aria-label', 'Settings');

    await page.getByRole('button', { name: 'Quit Peekle' }).click();
    await expect(page.getByRole('alertdialog')).toBeVisible();
    await expect(page.getByText('Are you sure you want to quit?')).toBeVisible();
    await expect(page.getByRole('button', { name: 'New session' })).toHaveCount(0);
    const setView = await calls(page, 'set_view');
    expect(setView.at(-1)).toEqual({ command: 'set_view', args: { view: 'Quit' } });

    // The shape shrank to the size of a question.
    await expect
      .poll(async () => (await page.locator('.shape').boundingBox())?.height ?? 999)
      .toBeLessThan(120);
  });

  test('No goes back to the list the question was asked over', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Sessions');

    await page.getByRole('button', { name: 'Quit Peekle' }).click();
    await page.getByRole('button', { name: 'No' }).click();

    const setView = await calls(page, 'set_view');
    expect(setView.at(-1)).toEqual({ command: 'set_view', args: { view: 'Sessions' } });
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    expect(await calls(page, 'quit_app')).toHaveLength(0);
  });

  test('⌥⌘Q over a resting island asks, and No lets it rest again', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');

    await view(page, 'Quit');
    await expect(page.getByRole('alertdialog')).toBeVisible();
    await page.getByRole('button', { name: 'No' }).click();

    const setView = await calls(page, 'set_view');
    expect(setView.at(-1)).toEqual({ command: 'set_view', args: { view: 'Collapsed' } });
  });

  test('Yes quits, once', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await view(page, 'Quit');

    await page.getByRole('button', { name: 'Yes' }).click();
    await expect(page.getByRole('button', { name: 'Yes' })).toBeDisabled();
    expect(await calls(page, 'quit_app')).toHaveLength(1);
  });
});
