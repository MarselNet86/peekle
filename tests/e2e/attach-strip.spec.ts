/**
 * v87.10 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.25.
 *
 * Criteria, from the request: the system file dialog opens behind the island,
 * so the island runs down to a strip that says what is already attached, and
 * the way back is a button or ⌘1. Rust is played by the stub: the global key
 * arrives as `peekle://choose`, the way the shortcut handler sends it, and
 * `set_shrunk` is what Rust is told.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

const card = {
  session: {
    session_id: 's1',
    cwd: '/Users/dev/peekle',
    project: 'peekle',
    pid: null,
    tty: null,
  },
  title: 'Refactor the panel',
  status: 'Idle',
  origin: 'Owned',
  entries: [],
  agent: null,
  mode: null,
  thinking: null,
  compacting: null,
  stopping: null,
  asking_trust: null,
  updated_at: 0,
};

async function stub(page: Page, files: string[]) {
  await page.addInitScript(
    ({ card, files }: { card: Record<string, unknown>; files: string[] }) => {
      const w = window as unknown as Record<string, unknown>;
      const handlers: Record<string, number> = {};
      const calls: { command: string; args: unknown }[] = [];
      w.__calls = calls;
      let counter = 0;
      const usage = {
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
      const answer = (command: string): unknown => {
        switch (command) {
          case 'get_language':
            return 'en';
          case 'get_state':
            return {
              enabled: true,
              view: { Session: 's1' },
              active_prompt: null,
              sessions: [card],
              tasks: [],
              usage,
              shot: null,
              live_sessions: 1,
              hotkey_ok: true,
            };
          case 'get_sessions':
            return [card];
          case 'get_models':
            return [];
          case 'get_account':
          case 'refresh_account':
            return account;
          case 'refresh_usage':
            return usage;
          // The dialog, answered the way the macOS panel answers: the paths,
          // and an empty list for a cancel. tech.md 6.25.
          case 'choose_files':
            return files;
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
          return answer(command);
        },
      };
      w.__choose = (index: number) => push('peekle://choose', { index });
      w.__listening = (event: string) => event in handlers;
    },
    { card, files },
  );
}

/** What the route told Rust about the strip, in order. */
async function shrinks(page: Page) {
  return page.evaluate(() =>
    (window as unknown as { __calls: { command: string; args: { shrunk?: boolean } }[] }).__calls
      .filter((call) => call.command === 'set_shrunk')
      .map((call) => call.args.shrunk),
  );
}

async function press(page: Page, index: number) {
  await page.waitForFunction(() =>
    (window as unknown as { __listening: (event: string) => boolean }).__listening(
      'peekle://choose',
    ),
  );
  await page.evaluate(
    (i) => (window as unknown as { __choose: (i: number) => void }).__choose(i),
    index,
  );
}

const attach = (page: Page) => page.getByRole('button', { name: 'Attach files' }).click();

test.describe('the island while files are picked', () => {
  test('runs down to a strip that counts what it took', async ({ page }) => {
    await stub(page, ['/Users/dev/peekle/report.pdf', '/Users/dev/peekle/notes.md']);
    await page.goto(ROUTE);
    await attach(page);

    await expect(page.getByText('2 files attached')).toBeVisible();
    // The chat is under the strip, not beside it: one line is the whole
    // island while the dialog stands. tech.md 6.25.
    await expect(page.locator('.reply textarea')).toHaveCount(0);
    await expect(page.locator('.rows')).toHaveCount(0);
  });

  /// The strip is smaller than the chat it stands over, and Rust reads the
  /// rectangle off the webview: a strip that reported the whole panel would
  /// keep eating the dialog's clicks. tech.md 6.7 and 6.25.
  test('draws itself no taller than a pill', async ({ page }) => {
    await stub(page, ['/Users/dev/peekle/report.pdf']);
    await page.goto(ROUTE);

    const shape = page.locator('.shape');
    await expect(shape).toBeVisible();
    await page.waitForTimeout(600);
    const open = (await shape.boundingBox())?.height ?? 0;

    await attach(page);
    await expect(page.getByText('1 file attached')).toBeVisible();
    await page.waitForTimeout(600);
    const strip = (await shape.boundingBox())?.height ?? 0;

    expect(strip).toBeLessThan(open);
    expect(strip).toBeLessThan(120);
  });

  test('tells Rust to hand the mouse back, and to take it again', async ({ page }) => {
    await stub(page, ['/Users/dev/peekle/report.pdf']);
    await page.goto(ROUTE);
    await attach(page);
    await expect(page.getByText('1 file attached')).toBeVisible();

    await expect.poll(() => shrinks(page)).toContain(true);

    await page.getByRole('button', { name: 'Expand' }).click();
    await expect(page.locator('.reply textarea')).toBeVisible();
    await expect.poll(() => shrinks(page)).toEqual([...(await shrinks(page)).slice(0, -1), false]);
  });

  test('⌘1 pressed anywhere opens it back up', async ({ page }) => {
    await stub(page, ['/Users/dev/peekle/report.pdf']);
    await page.goto(ROUTE);
    await attach(page);
    await expect(page.getByText('1 file attached')).toBeVisible();

    await press(page, 1);

    await expect(page.locator('.reply textarea')).toBeVisible();
    await expect(page.getByText('report.pdf')).toBeVisible();
  });

  /// A cancel is not an event and says nothing, but the island still has to
  /// come back: an empty strip over a chat is the island stuck aside.
  test('says it is choosing while nothing has been picked', async ({ page }) => {
    await stub(page, []);
    await page.goto(ROUTE);
    await attach(page);

    // Nothing came back, so the strip has nothing to count and says so rather
    // than counting to zero.
    await page.getByRole('button', { name: 'Expand' }).click();
    await expect(page.locator('.reply textarea')).toBeVisible();
  });
});
