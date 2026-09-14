/**
 * Switching between chats, hard and fast, with the feed changing under it the
 * way hooks change it. The island froze on the owner's Mac doing exactly this,
 * and a webview that has stopped every effect gives no error to anyone but the
 * page itself, so the page is asked. tech.md 6.12.
 */

import { expect, test } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

type Card = Record<string, unknown>;

/** A long dialogue: every kind of entry the feed draws, in the proportions a
 * real turn has, with Markdown in what the agent said. */
function dialogue(id: string, turns: number, salt: number): Card {
  const entries: unknown[] = [];
  let at = 1_789_000_000_000 + salt;
  for (let turn = 0; turn < turns; turn += 1) {
    entries.push({
      id: `${id}-u${turn}`,
      kind: 'User',
      text: `Do the thing number ${turn} in ${id}`,
      tool: null,
      detail: null,
      state: 'Ok',
      at: (at += 1000),
    });
    for (let call = 0; call < 3; call += 1) {
      entries.push({
        id: `${id}-t${turn}-${call}`,
        kind: 'Tool',
        text: `Read src/file${turn}-${call}.ts`,
        tool: 'Read',
        detail: `line ${salt}\n`.repeat(20),
        state: 'Ok',
        at: (at += 300),
      });
    }
    entries.push({
      id: `${id}-a${turn}`,
      kind: 'Assistant',
      text:
        `## Step ${turn}\n\nDone with **${id}** at pass ${salt}: \`file${turn}.ts\` reads fine.\n\n` +
        `- one thing\n- another thing\n  - nested\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n` +
        '```ts\nconst x = 1;\n```\n' +
        'And a longer paragraph. '.repeat(30),
      tool: null,
      detail: null,
      state: 'Ok',
      at: (at += 800),
    });
  }
  return {
    session: { session_id: id, cwd: `/Users/dev/${id}`, project: id, pid: null, tty: null },
    title: `Chat ${id}`,
    status: salt % 2 === 0 ? 'Working' : 'Idle',
    origin: 'Observed',
    entries,
    agent: {
      model: 'claude-opus-5',
      label: 'Opus 5',
      effort: 'High',
      levels: ['Low', 'Medium', 'High', 'XHigh', 'Max'],
      context_tokens: 120000,
      context_window: 1000000,
      context_pct: 12,
    },
    mode: null,
    thinking: null,
    compacting: null,
    stopping: null,
    asking_trust: null,
    updated_at: at,
  };
}

function cards(salt: number): Card[] {
  return [dialogue('s1', 40, salt), dialogue('s2', 40, salt + 1), dialogue('s3', 40, salt + 2)];
}

async function stub(page: import('@playwright/test').Page, initial: Card[]) {
  await page.addInitScript(
    ({ initial }: { initial: Card[] }) => {
      const handlers: Record<string, number> = {};
      let counter = 0;
      const w = window as unknown as Record<string, unknown>;
      const usage = {
        windows: [
          { window: 'FiveHour', used_pct: 43, resets_at: null },
          { window: 'SevenDay', used_pct: 35, resets_at: null },
        ],
        source: 'Account',
        reason: null,
        fetched_at: 0,
        keychain_granted: true,
        retry_after_ms: null,
      };
      const answers: Record<string, unknown> = {
        get_language: 'en',
        get_state: {
          enabled: true,
          view: 'Sessions',
          active_prompt: null,
          sessions: initial,
          tasks: [],
          usage,
          shot: null,
          live_sessions: 1,
          hotkey_ok: true,
        },
        get_sessions: initial,
        get_models: [],
        get_defaults: null,
        refresh_usage: usage,
        choose_files: [],
      };
      const push = (event: string, payload: unknown) => {
        const id = handlers[event];
        const handler = w[`_${id}`] as ((p: unknown) => void) | undefined;
        handler?.({ event, id: 1, payload });
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
          // Rust answers a view intent with the view event, in the same tick
          // it would take over IPC. tech.md section 8.
          if (command === 'set_view') queueMicrotask(() => push('peekle://view', args.view));
          return command in answers ? answers[command] : null;
        },
      };
      w.__sessions = (next: unknown) => push('peekle://sessions', next);
      w.__view = (next: unknown) => push('peekle://view', next);
    },
    { initial },
  );
}

test.describe('switching between chats', () => {
  /// Thirty switches in three seconds while the feed is replaced under them
  /// every hundred milliseconds. The page must throw nothing, and it must
  /// still be drawing the chat it was last asked for.
  test('the island survives fast switching under a changing feed', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(message.text());
    });

    await stub(page, cards(0));
    await page.goto(ROUTE);
    // The first route of a run waits out the dev server's compile.
    await expect(page.getByText('Chat s1')).toBeVisible({ timeout: 30_000 });

    const ids = ['s1', 's2', 's3'];
    for (let round = 0; round < 30; round += 1) {
      const id = ids[round % ids.length];
      await page.evaluate(
        ({ id, next }) => {
          const w = window as unknown as {
            __view: (v: unknown) => void;
            __sessions: (c: unknown) => void;
          };
          w.__view({ Session: id });
          w.__sessions(next);
        },
        { id, next: cards(round + 1) },
      );
      await page.waitForTimeout(100);
    }

    // The last switch was to s3, and its feed is what stands.
    await expect(
      page.locator('.feed').getByText('Done with s3', { exact: false }).first(),
    ).toBeVisible();
    // Back to the list, which has to answer as fast as it ever did.
    await page.evaluate(() =>
      (window as unknown as { __view: (v: unknown) => void }).__view('Sessions'),
    );
    await expect(page.getByText('Chat s2')).toBeVisible({ timeout: 2000 });
    expect(errors).toEqual([]);
  });
});
