/**
 * v87.5 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.14.
 *
 * Criteria, from the owner's request: the question window in the new style,
 * every row wearing ⌘ and its digit, and ⌘ with a digit picking that row.
 * Rust is played by the stub: the global key reaches the route as
 * `peekle://choose`, the way the shortcut handler sends it.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

const question = {
  id: 'q1',
  kind: 'Question',
  session: { session_id: 's1', cwd: '/Users/dev/api', project: 'api', pid: null, tty: null },
  title: 'Claude asks',
  tool: 'AskUserQuestion',
  last_message: null,
  detail: null,
  options: [],
  questions: [
    {
      header: 'Deploy',
      question: 'Which deployment target?',
      options: [
        { label: 'Production', description: null },
        { label: 'Staging', description: null },
        { label: 'Local only', description: null },
      ],
      multi_select: false,
    },
  ],
  allow_free_text: false,
  created_at: 0,
  expires_at: 0,
};

const card = {
  session: question.session,
  title: 'Deploy the API',
  status: 'Working',
  origin: 'Observed',
  entries: [
    {
      id: 'u1',
      kind: 'User',
      text: 'Ship the API',
      tool: null,
      detail: null,
      state: 'Ok',
      at: 1_789_000_000_000,
    },
  ],
  agent: null,
  mode: null,
  thinking: null,
  compacting: null,
  stopping: null,
  asking_trust: null,
  updated_at: 0,
};

async function stub(page: Page, view: unknown = { Session: 's1' }) {
  await page.addInitScript(
    ({ question, card, view }) => {
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
              view,
              active_prompt: question,
              sessions: [card],
              tasks: [],
              usage: snapshot,
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
      w.__close = () =>
        push('peekle://prompt-close', { prompt_id: 'q1', outcome: 'AnsweredElsewhere' });
      w.__listening = (event: string) => event in handlers;
    },
    { question, card, view },
  );
}

test.describe('the question window', () => {
  test('says who asks and wears ⌘ and a digit on every row', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);

    await expect(page.getByText('Claude asks')).toBeVisible();
    await expect(page.getByText('Which deployment target?')).toBeVisible();
    await expect(page.locator('.option .key')).toHaveText(['⌘1', '⌘2', '⌘3', '⌘4']);
  });

  /// v87.5.1: the answer cards are styled by a class of their own. They used
  /// to be `.row`, globally, and every other row in the product -- the session
  /// list, the settings, the feed -- took their green edge. tech.md 9.
  test('lends its card style to no other row in the island', async ({ page }) => {
    // The session list, where every chat is a `.row` of its own. The card
    // styles are loaded with the route whether or not a question is showing.
    await stub(page, 'Sessions');
    await page.goto(ROUTE);
    await expect(page.locator('.row').first()).toBeVisible();

    const rows = await page.evaluate(() => {
      const green = /48,\s*209,\s*88/;
      const styles = [...document.querySelectorAll('.row')].map((row) => getComputedStyle(row));
      return {
        count: styles.length,
        green: styles.filter(
          (style) => green.test(style.borderTopColor) || green.test(style.backgroundColor),
        ).length,
      };
    });
    expect(rows.count).toBeGreaterThan(0);
    expect(rows.green).toBe(0);
  });

  /// v87.6: the question and its answers, and nothing of the chat. The feed
  /// comes back the moment the question goes. tech.md 6.14.
  test('shows the question alone and gives the chat back once it goes', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);

    await expect(page.getByText('Which deployment target?')).toBeVisible();
    await expect(page.getByText('Ship the API')).toHaveCount(0);
    // The band with the project and the usage stays.
    await expect(page.locator('.head .project')).toHaveText('api');

    await page.waitForFunction(() =>
      (window as unknown as { __listening: (event: string) => boolean }).__listening(
        'peekle://prompt-close',
      ),
    );
    await page.evaluate(() => (window as unknown as { __close: () => void }).__close());

    await expect(page.getByText('Which deployment target?')).toHaveCount(0);
    await expect(page.getByText('Ship the API')).toBeVisible();
  });

  test('⌘ and a digit pressed anywhere answers with that row', async ({ page }) => {
    await stub(page);
    await page.goto(ROUTE);
    await expect(page.getByText('Which deployment target?')).toBeVisible();
    await page.waitForFunction(() =>
      (window as unknown as { __listening: (event: string) => boolean }).__listening(
        'peekle://choose',
      ),
    );

    await page.evaluate(() => (window as unknown as { __choose: (i: number) => void }).__choose(2));

    await expect
      .poll(() =>
        page.evaluate(() =>
          (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls.filter(
            (call) => call.command === 'answer_prompt',
          ),
        ),
      )
      .toEqual([
        {
          command: 'answer_prompt',
          args: {
            answer: {
              prompt_id: 'q1',
              choice: null,
              text: null,
              answers: [{ question: 'Which deployment target?', labels: ['Staging'] }],
            },
          },
        },
      ]);
  });
});
