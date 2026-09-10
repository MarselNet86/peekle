/**
 * The island route with a stub where Tauri would be.
 *
 * The route is the one place several of these rules live, and it cannot be
 * rendered in a unit test: it talks to the bridge on mount. So the bridge is
 * stubbed here -- commands answer with fixtures, and `window.__sessions`
 * pushes a `peekle://sessions` event the way Rust does after every hook.
 * tech.md section 8.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';

type Card = Record<string, unknown>;

const session = {
  session_id: 's1',
  cwd: '/Users/dev/peekle',
  project: 'peekle',
  pid: null,
  tty: null,
};

/** A chat another app is running: its model and effort are set there, so the
 * row only reads and a press on it is answered by a note. tech.md 6.15. */
function observed(entries: unknown[] = [], id = 's1'): Card {
  return {
    session: { ...session, session_id: id },
    title: 'Refactor the panel',
    status: 'Working',
    origin: 'Observed',
    entries,
    // A chat that has answered once: the row under the field stands on what
    // the transcript said, and with nothing there it is not drawn at all.
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
    updated_at: 0,
  };
}

function say(id: string, text: string, at: number) {
  return { id, kind: 'User', text, tool: null, detail: null, state: 'Ok', at };
}

/** Everything the route asks for on mount, plus a way to push events at it. */
async function stub(page: Page, cards: Card[]) {
  await page.addInitScript((cards) => {
    const handlers: Record<string, number> = {};
    let counter = 0;
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
      get_state: {
        enabled: true,
        view: { Session: 's1' },
        active_prompt: null,
        sessions: cards,
        tasks: [],
        usage,
        shot: null,
        live_sessions: 1,
        hotkey_ok: true,
      },
      get_sessions: cards,
      get_models: [],
      get_defaults: null,
      refresh_usage: usage,
    };

    // The shape @tauri-apps/api talks to. `transformCallback` hands the
    // runtime a number and expects the callback back under that name.
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
      transformCallback: (cb: unknown) => {
        const id = ++counter;
        (window as unknown as Record<string, unknown>)[`_${id}`] = cb;
        return id;
      },
      convertFileSrc: (path: string) => path,
      invoke: async (command: string, args: Record<string, unknown>) => {
        if (command === 'plugin:event|listen') {
          handlers[args.event as string] = args.handler as number;
          return 1;
        }
        if (command.startsWith('plugin:event|')) return 1;
        return command in answers ? answers[command] : null;
      },
    };

    const push = (event: string, payload: unknown) => {
      const id = handlers[event];
      const handler = (window as unknown as Record<string, (payload: unknown) => void>)[`_${id}`];
      handler?.({ event, id: 1, payload });
    };
    (window as unknown as Record<string, unknown>).__sessions = (next: unknown) =>
      push('peekle://sessions', next);
    // The view is Rust's, and this is how it arrives. tech.md section 8.
    (window as unknown as Record<string, unknown>).__view = (next: unknown) =>
      push('peekle://view', next);
  }, cards);
}

test.describe('the island route', () => {
  /// The answer to a press is spent by reading it, and by nothing else. It
  /// used to be cleared by the next card that arrived: the route watched the
  /// card for "the session changed", and a card is a new object on every
  /// hook. While a turn ran, hooks arrive on every tool call, so the note the
  /// user had just asked for was wiped within a second. tech.md 6.15.
  test('a note stands while the agent keeps working', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));

    await stub(page, [observed([say('u1', 'go on', 1_789_000_000_000)])]);
    await page.goto(ROUTE);

    const chip = page.locator('.chip').first();
    await expect(chip).toBeVisible();
    await chip.click();

    const note = page.getByText('Another app is running this chat', { exact: false });
    await expect(note).toBeVisible();

    // A hook lands: Rust replaces the whole list, so every card is a new
    // object even when nothing about this one changed.
    await page.evaluate(() => {
      (window as unknown as { __sessions: (cards: unknown) => void }).__sessions([
        {
          session: {
            session_id: 's1',
            cwd: '/Users/dev/peekle',
            project: 'peekle',
            pid: null,
            tty: null,
          },
          title: 'Refactor the panel',
          status: 'Working',
          origin: 'Observed',
          entries: [
            {
              id: 'u1',
              kind: 'User',
              text: 'go on',
              tool: null,
              detail: null,
              state: 'Ok',
              at: 1_789_000_000_000,
            },
            {
              id: 'a1',
              kind: 'Assistant',
              text: 'a line the hook brought',
              tool: null,
              detail: null,
              state: 'Ok',
              at: 1_789_000_001_000,
            },
          ],
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
          updated_at: 2,
        },
      ]);
    });

    // The event landed -- the feed is drawing the card that came with it...
    await expect(page.getByText('a line the hook brought')).toBeVisible();
    // ...and the answer to the press is still on screen.
    await expect(note).toBeVisible();
    expect(errors).toEqual([]);
  });

  /// And it is spent by leaving: the answer belongs to the row that was
  /// pressed, and that row belongs to the session on screen. tech.md 6.15.
  test('a note is left behind when the session changes', async ({ page }) => {
    await stub(page, [
      observed([say('u1', 'go on', 1_789_000_000_000)]),
      observed([say('u2', 'over here', 1_789_000_000_000)], 's2'),
    ]);
    await page.goto(ROUTE);

    await page.locator('.chip').first().click();
    const note = page.getByText('Another app is running this chat', { exact: false });
    await expect(note).toBeVisible();

    await page.evaluate(() => {
      (window as unknown as { __view: (view: unknown) => void }).__view({ Session: 's2' });
    });

    await expect(page.getByText('over here')).toBeVisible();
    await expect(note).toHaveCount(0);
  });
});
