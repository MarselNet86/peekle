/**
 * v87 acceptance, end to end on the island route with a stub where Tauri would
 * be. tech.md 6.30.
 *
 * Rust is played by the stub: `peekle://update` carries the state the check
 * settled on, `set_view` answers with `peekle://view` the way Rust does, and
 * the question is pushed the way the finished check pushes it. Every command
 * the route sends is kept on `window.__calls`.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

const UPDATE = {
  version: '0.1.2',
  notes_url: 'https://github.com/MarselNet86/peekle/releases/tag/v0.1.2',
  size: 104_857_600,
  install: 'Bundle',
};

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
            view: 'Collapsed',
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
          setTimeout(() => push('peekle://view', args?.view), 0);
          return null;
        // "Later" is Rust's to answer: it folds the island itself.
        case 'dismiss_update':
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
    w.__update = (state: unknown) => push('peekle://update', state);
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

async function ready(page: Page, event: string) {
  await page.waitForFunction(
    (name) =>
      (window as unknown as { __listening: (event: string) => boolean }).__listening(
        name as string,
      ),
    event,
  );
}

async function view(page: Page, next: unknown) {
  await ready(page, 'peekle://view');
  await page.evaluate(
    (value) => (window as unknown as { __view: (v: unknown) => void }).__view(value),
    next,
  );
}

async function update(page: Page, state: unknown) {
  await ready(page, 'peekle://update');
  await page.evaluate(
    (value) => (window as unknown as { __update: (v: unknown) => void }).__update(value),
    state,
  );
}

/** The check finished with a file on disk and Rust raised the question. */
async function offer(page: Page, state: unknown = { Ready: UPDATE }) {
  await update(page, state);
  await view(page, 'Update');
}

test.describe('the update question', () => {
  test('stands over a resting island with the version and the size', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page);

    await expect(page.getByRole('alertdialog')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Update to 0.1.2?' })).toBeVisible();
    await expect(page.getByText('105 MB downloaded and ready to install.')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Install' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Later' })).toBeVisible();

    // The island opened just far enough to ask, the same as a permission does.
    await expect
      .poll(async () => (await page.locator('.shape').boundingBox())?.height ?? 999)
      .toBeLessThan(120);
  });

  /// Checking and downloading are the island staying quiet on purpose: a
  /// question you then have to wait out is worse than no question. tech.md 6.30.
  test('says nothing while the check runs and while the file comes down', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');

    await update(page, 'Checking');
    await expect(page.getByRole('alertdialog')).toHaveCount(0);

    await update(page, { Downloading: UPDATE });
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    expect(await calls(page, 'set_view')).toHaveLength(0);
  });

  /// A failed check is a line in the log, not a banner over somebody's day.
  test('says nothing when the check fails', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');

    for (const reason of ['Offline', 'RateLimited', 'NoAsset', 'Download']) {
      await update(page, { Failed: reason });
      await expect(page.getByRole('alertdialog')).toHaveCount(0);
    }
  });

  test('Later folds the island and asks Rust to put the version away', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page);

    await page.getByRole('button', { name: 'Later' }).click();

    expect(await calls(page, 'dismiss_update')).toHaveLength(1);
    expect(await calls(page, 'install_update')).toHaveLength(0);
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
  });

  test('Escape is Later', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page);

    await page.keyboard.press('Escape');

    expect(await calls(page, 'dismiss_update')).toHaveLength(1);
  });

  /// Install goes once. The buttons lock while Rust hands the file to Finder
  /// and stands aside, so a second press cannot start a second install.
  test('Install installs, once', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page);

    await page.getByRole('button', { name: 'Install' }).click();
    await expect(page.getByRole('button', { name: 'Install' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Later' })).toBeDisabled();

    expect(await calls(page, 'install_update')).toHaveLength(1);
  });

  /// The version in the title is the way to the release page, and the address
  /// never leaves the webview: the command takes no argument. tech.md 6.30.
  test('the version opens the release notes and names no address', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page);

    await page.getByRole('button', { name: 'Update to 0.1.2?' }).click();

    const notes = await calls(page, 'open_update_notes');
    expect(notes).toHaveLength(1);
    // Empty, not merely free of an address: the webview has nothing to say
    // about where the page is.
    expect(notes[0]).toEqual({ command: 'open_update_notes', args: {} });
  });

  /// A copy Homebrew owns is never handed a file. tech.md 6.30.
  test('a Homebrew copy is offered the command instead of a file', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page, {
      Ready: { ...UPDATE, size: 0, install: 'Homebrew' },
    });

    await expect(page.getByText('brew upgrade --cask peekle')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Copy command' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Install' })).toHaveCount(0);
  });

  /// Every string a person sees has both halves. tech.md 6.28.
  test('asks in Russian when that is the language', async ({ page }) => {
    await stub(page);
    await page.addInitScript(() => {
      const w = window as unknown as Record<string, unknown>;
      const inner = (w.__TAURI_INTERNALS__ as { invoke: unknown }).invoke as (
        command: string,
        args: Record<string, unknown>,
      ) => Promise<unknown>;
      (w.__TAURI_INTERNALS__ as { invoke: unknown }).invoke = async (
        command: string,
        args: Record<string, unknown>,
      ) => (command === 'get_language' ? 'ru' : inner(command, args));
    });
    await page.goto(ROUTE);
    await view(page, 'Collapsed');
    await offer(page);

    await expect(page.getByRole('button', { name: 'Установить Peekle 0.1.2?' })).toBeVisible();
    await expect(page.getByText('105 MB скачано, можно ставить.')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Установить', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Позже' })).toBeVisible();
  });
});
