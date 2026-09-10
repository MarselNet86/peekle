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
    stopping: null,
    updated_at: 0,
  };
}

/** A chat Peekle has just started: nothing said in it, nothing running.
 * tech.md 6.12. */
function fresh(status = 'Idle', entries: unknown[] = []): Card {
  return { ...observed(entries), status, agent: null };
}

/** A chat Peekle aimed and nobody has spoken in: the folder is still a choice.
 * tech.md 6.23. */
function aimed(cwd: string, id = 's1', entries: unknown[] = []): Card {
  const project = cwd.split('/').filter(Boolean).at(-1) ?? '';
  const card = fresh('Idle', entries) as Card & { session: Record<string, unknown> };
  return {
    ...card,
    session: { ...card.session, session_id: id, cwd, project },
    origin: 'Owned',
  };
}

function say(id: string, text: string, at: number) {
  return { id, kind: 'User', text, tool: null, detail: null, state: 'Ok', at };
}

/** Everything the route asks for on mount, plus a way to push events at it.
 * Every call the route makes is kept on `window.__calls`, because what a
 * button sends is as much the contract as what it looks like. */
async function stub(page: Page, cards: Card[], view: unknown = { Session: 's1' }) {
  await page.addInitScript(
    ({ cards, view }: { cards: Card[]; view: unknown }) => {
      const handlers: Record<string, number> = {};
      const calls: { command: string; args: unknown }[] = [];
      (window as unknown as Record<string, unknown>).__calls = calls;
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
          view,
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
          calls.push({ command, args: args ?? null });
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
    },
    { cards, view },
  );
}

/** Waits out the opening spring and hands back the box it settled on: the
 * island grows into place over a few frames, and a measurement taken while it
 * is still growing says nothing about what moved. */
async function settled(locator: ReturnType<Page['locator']>) {
  let last: { x: number; y: number; width: number; height: number } | null = null;
  await expect
    .poll(async () => {
      const box = await locator.boundingBox();
      const still =
        !!box &&
        !!last &&
        Math.abs(box.y - last.y) < 0.5 &&
        Math.abs(box.height - last.height) < 0.5;
      last = box;
      return still;
    })
    .toBe(true);
  return last!;
}

test.describe('the island route', () => {
  /// While a compact runs the feed carries one clock and one only: the
  /// compact's. The turn's own working line stands down, because the agent is
  /// not working -- the CLI is -- and two clocks over one pause are two
  /// answers to one question. The rule lives in the route, so this is the
  /// only place it can be read. tech.md 6.21.
  test('a compact leaves one clock running in the dialogue', async ({ page }) => {
    const working = (compacting: unknown) => ({
      ...observed([
        say('u1', 'go on', 1_789_000_000_000),
        {
          id: 't1',
          kind: 'Tool',
          text: 'Read tech.md',
          tool: 'Read',
          detail: null,
          state: 'Ok',
          at: 1_789_000_001_000,
        },
      ]),
      status: 'Working',
      compacting,
    });

    await stub(page, [working({ since: Date.now() - 12_000, manual: true })]);
    await page.goto(ROUTE);

    const running = page.locator('.work.running');
    await expect(running).toHaveCount(1);
    await expect(running).toContainText('Compacting');

    // And with no compact the same card runs the turn's line instead.
    await page.evaluate((card) => {
      (window as unknown as { __sessions: (cards: unknown) => void }).__sessions([card]);
    }, working(null));

    await expect(page.locator('.work.running')).toHaveCount(1);
    await expect(page.locator('.work.running')).not.toContainText('Compacting');
  });

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
          stopping: null,
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
  /// A chat with nothing in it stands under the sign rather than over half a
  /// window of black: an empty dialogue that says nothing at all reads as a
  /// screen that did not finish drawing. tech.md 6.12 and 9.
  test('an empty chat carries the sign, and the first line takes it away', async ({ page }) => {
    await stub(page, [fresh()]);
    await page.goto(ROUTE);

    const sign = page.getByRole('img', { name: 'Nothing said in this chat yet' });
    await expect(sign).toBeVisible();
    // The strokes say whose window this is; the line says what it waits for.
    const start = page.getByText("Let's begin");
    await expect(start).toBeVisible();

    // The first thing said arrives on a hook, the way everything does.
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
          status: 'Idle',
          origin: 'Observed',
          entries: [
            {
              id: 'u1',
              kind: 'User',
              text: 'the first word',
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
          updated_at: 2,
        },
      ]);
    });

    await expect(page.getByText('the first word')).toBeVisible();
    await expect(sign).toHaveCount(0);
    await expect(start).toHaveCount(0);
  });

  /// A turn that has called nothing yet is not an empty chat: the work line
  /// is in the feed from its first second, and two answers to "is anything
  /// happening" is one too many. tech.md 6.12.
  test('a chat that is working carries the work line instead', async ({ page }) => {
    await stub(page, [fresh('Working')]);
    await page.goto(ROUTE);

    await expect(page.locator('.work').first()).toBeVisible();
    await expect(page.getByRole('img', { name: 'Nothing said in this chat yet' })).toHaveCount(0);
  });

  /// A chat that has not begun is still choosing where the agent will work,
  /// and the choice stands where the folder is already named: the top left of
  /// the dialogue. The menu hangs below it, because above it is the top edge
  /// of the screen -- which is layout, and layout is measured here. tech.md
  /// 6.23.
  test('a chat that has not begun chooses its folder', async ({ page }) => {
    await stub(page, [
      aimed('/Users/dev/peekle'),
      aimed('/Users/dev/site', 's2'),
      aimed('/Users/dev/notes', 's3'),
    ]);
    await page.goto(ROUTE);

    const chip = page.getByRole('button', { name: 'peekle' });
    await expect(chip).toBeVisible();
    await chip.click();

    const menu = page.getByRole('menu');
    const site = menu.getByRole('menuitemradio', { name: /site/ });
    await expect(site).toBeVisible();
    // Every folder the island knows, and the way to the rest of the disk last.
    await expect(menu.getByText('Open folder…')).toBeVisible();
    // The tick stands on the folder in force.
    await expect(menu.getByRole('menuitemradio', { name: /peekle/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );

    // Below the button, and on a ground of its own: the ground lived in the
    // flipped rule alone until v80.9, so a menu opening left had none at all.
    const under = (await menu.boundingBox())!;
    const over = (await chip.boundingBox())!;
    expect(under.y).toBeGreaterThan(over.y);
    const ground = await menu.evaluate((node) => getComputedStyle(node).backgroundColor);
    expect(ground).not.toBe('rgba(0, 0, 0, 0)');

    await site.click();
    const calls = await page.evaluate(
      () => (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls,
    );
    const aimedAt = calls.filter((call) => call.command === 'set_session_cwd');
    expect(aimedAt).toHaveLength(1);
    expect(aimedAt[0].args).toEqual({ sessionId: 's1', cwd: '/Users/dev/site' });
  });

  /// And once something has been said, the agent is living in that folder:
  /// the name goes back to being a name. tech.md 6.23.
  test('a chat that has begun only names its folder', async ({ page }) => {
    await stub(page, [aimed('/Users/dev/peekle', 's1', [say('u1', 'go on', 1_789_000_000_000)])]);
    await page.goto(ROUTE);

    await expect(page.getByText('go on')).toBeVisible();
    await expect(page.locator('.head .picker-menu')).toHaveCount(0);
    await expect(page.locator('.head .project')).toContainText('peekle');
  });

  /// The way to the developer stands in the corner of the list, and its line
  /// is a layer rather than a row: a hint that pushes the list down moves the
  /// thing the reader was about to press. Layout is why this lives here --
  /// jsdom measures nothing. tech.md 6.22.
  test('the bug button stands beside the gear and its line moves no row', async ({ page }) => {
    await stub(page, [fresh()], 'Sessions');
    await page.goto(ROUTE);

    const bug = page.getByRole('button', { name: 'Report a bug' });
    const gear = page.getByRole('button', { name: 'Settings' });
    await expect(bug).toBeVisible();
    const rows = page.locator('.rows');
    const before = await settled(rows);

    // Beside the gear, and on the inside of it: the gear stood in the very
    // corner first, and a button that arrived later does not take its place.
    const beside = (await bug.boundingBox())!;
    const corner = (await gear.boundingBox())!;
    expect(beside.x).toBeLessThan(corner.x);
    expect(Math.abs(beside.y - corner.y)).toBeLessThan(1);

    const hint = page.getByRole('tooltip');
    await expect(hint).toHaveCount(0);

    await bug.hover();
    await expect(hint).toContainText('Tell the developer what broke');
    expect((await rows.boundingBox())!.y).toBe(before.y);
    // And it stays inside the shape rather than hanging off its side.
    const line = (await hint.boundingBox())!;
    const shape = (await page.locator('.shape').boundingBox())!;
    expect(line.x).toBeGreaterThanOrEqual(shape.x);
    expect(line.x + line.width).toBeLessThanOrEqual(shape.x + shape.width);

    await gear.hover();
    await expect(hint).toHaveCount(0);

    // What it sends is the contract: the address is Rust's, so the press
    // carries no address of its own. tech.md 6.22.
    await bug.click();
    const calls = await page.evaluate(
      () => (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls,
    );
    const sent = calls.filter((call) => call.command === 'open_bug_report');
    expect(sent).toHaveLength(1);
    expect(Object.keys(sent[0].args as Record<string, unknown>)).toEqual([]);
  });
});
