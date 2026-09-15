/**
 * v87.8 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.7.
 *
 * Criteria, from the request: Allow and Deny answer to key combinations the way
 * a question's rows do. The owner chose ⌘1 and ⌘2 from anywhere; they follow
 * the buttons in the order they stand, Deny on the left and Allow on the right.
 * Rust is played by the stub: the global key reaches the route as
 * `peekle://choose`, the way the shortcut handler sends it.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

const permission = {
  id: 'p1',
  kind: 'Permission',
  session: { session_id: 's1', cwd: '/Users/dev/api', project: 'api', pid: null, tty: null },
  title: 'Bash wants to run',
  tool: 'Bash',
  last_message: null,
  detail: 'git push origin main',
  options: [
    { id: 'allow_once', label: 'Allow', hint: null, kind: 'AllowOnce' },
    { id: 'deny', label: 'Deny', hint: null, kind: 'Deny' },
  ],
  questions: [],
  allow_free_text: true,
  created_at: Date.now(),
  expires_at: Date.now() + 60_000,
};

async function stub(page: Page) {
  await page.addInitScript((request) => {
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
    const answer = (command: string): unknown => {
      switch (command) {
        case 'get_language':
          return 'en';
        case 'get_state':
          return {
            enabled: true,
            view: 'Ask',
            active_prompt: { ...request, created_at: Date.now(), expires_at: Date.now() + 60_000 },
            sessions: [],
            tasks: [],
            usage: snapshot,
            shot: null,
            live_sessions: 1,
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
        calls.push({ command, args: args ?? null });
        return answer(command);
      },
    };
    w.__choose = (index: number) => push('peekle://choose', { index });
    w.__listening = (event: string) => event in handlers;
  }, permission);
}

async function answers(page: Page) {
  return page.evaluate(() =>
    (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls.filter(
      (call) => call.command === 'answer_prompt',
    ),
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

test.describe('the permission panel', () => {
  test('wears ⌘1 on Deny and ⌘2 on Allow', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);

    await expect(page.getByRole('button', { name: /Deny/ })).toContainText('⌘1');
    await expect(page.getByRole('button', { name: /Allow/ })).toContainText('⌘2');
  });

  test('⌘2 pressed anywhere allows, once', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await expect(page.getByRole('button', { name: /Allow/ })).toBeVisible();

    await press(page, 2);
    await expect
      .poll(() => answers(page))
      .toEqual([
        {
          command: 'answer_prompt',
          args: { answer: { prompt_id: 'p1', choice: 'allow_once', text: null, answers: [] } },
        },
      ]);
  });

  test('⌘1 pressed anywhere denies', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await expect(page.getByRole('button', { name: /Deny/ })).toBeVisible();

    await press(page, 1);
    await expect
      .poll(() => answers(page))
      .toEqual([
        {
          command: 'answer_prompt',
          args: { answer: { prompt_id: 'p1', choice: 'deny', text: null, answers: [] } },
        },
      ]);
  });
});
