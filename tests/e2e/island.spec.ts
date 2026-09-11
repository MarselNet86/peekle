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
    asking_trust: null,
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
        // A shot has to actually load, or the row falls back to its path and
        // there is nothing to press. Anything else keeps its path.
        convertFileSrc: (path: string) =>
          path.includes('/peekle/shots/')
            ? 'data:image/svg+xml,' +
              encodeURIComponent(
                '<svg xmlns="http://www.w3.org/2000/svg" width="1172" height="246">' +
                  '<rect width="1172" height="246" fill="rgb(38,42,50)"/></svg>',
              )
            : path,
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
          asking_trust: null,
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
          asking_trust: null,
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
  /// and the choice stands over the field, on its left edge: the folder is
  /// part of what is about to be said. Where it stands is layout, and layout
  /// is measured here. tech.md 6.23.
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

    // Over the field and on its left edge, and on a ground of its own: the
    // ground lived in the flipped rule alone until v80.9, so a menu opening
    // left had none at all.
    const button = (await chip.boundingBox())!;
    const field = (await page.locator('.reply textarea, .reply input').first().boundingBox())!;
    expect(button.y + button.height).toBeLessThanOrEqual(field.y);
    expect(Math.abs(button.x - field.x)).toBeLessThan(24);
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
    await expect(page.locator('.aim')).toHaveCount(0);
    await expect(page.locator('.head .project')).toContainText('peekle');
  });

  /// A page of one's own words pushes the answer to them off the island, and
  /// it is the answer the person is waiting for. So a long message of one's
  /// own folds: six lines, faded rather than cut, and the whole of it one
  /// press away. Layout decides whether it folds at all, so it is read here.
  /// tech.md 6.12.
  test('a long message of your own is folded, and opens on a press', async ({ page }) => {
    const long = Array.from(
      { length: 12 },
      (_, line) => `строка ${line + 1} этой длинной реплики`,
    ).join('\n');
    await stub(page, [observed([say('u1', long, 1_789_000_000_000)])]);
    await page.goto(ROUTE);

    const body = page.locator(".line[data-kind='User'] .body");
    await expect(body).toHaveClass(/folded/);
    // Folded means some of it is out of view, and the fade is what says so.
    const folded = await body.evaluate((node) => ({
      hidden: node.scrollHeight > node.clientHeight + 4,
      faded: getComputedStyle(node).webkitMaskImage !== 'none',
    }));
    expect(folded).toEqual({ hidden: true, faded: true });

    await page.getByRole('button', { name: 'Show more' }).click();
    await expect(body).not.toHaveClass(/folded/);
    const whole = await body.evaluate((node) => node.scrollHeight <= node.clientHeight + 4);
    expect(whole).toBe(true);

    // And it folds back, so one long message cannot own the window.
    await page.getByRole('button', { name: 'Show less' }).click();
    await expect(body).toHaveClass(/folded/);
  });

  /// Short of that, nothing is folded and nothing is offered: a control that
  /// does nothing is worse than no control.
  test('a message that fits carries no control', async ({ page }) => {
    await stub(page, [observed([say('u1', 'go on', 1_789_000_000_000)])]);
    await page.goto(ROUTE);

    await expect(page.getByText('go on')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Show more' })).toHaveCount(0);
  });

  /// An answer is never folded, however long: reading it is what the window
  /// is for. tech.md 6.12.
  test('an answer is never folded', async ({ page }) => {
    const long = Array.from({ length: 14 }, (_, line) => `строка ${line + 1} длинного ответа`).join(
      '\n',
    );
    await stub(page, [
      observed([
        {
          id: 'a1',
          kind: 'Assistant',
          text: long,
          tool: null,
          detail: null,
          state: 'Ok',
          at: 1_789_000_000_000,
        },
      ]),
    ]);
    await page.goto(ROUTE);

    await expect(page.locator(".line[data-kind='Assistant'] .body")).not.toHaveClass(/folded/);
    await expect(page.getByRole('button', { name: 'Show more' })).toHaveCount(0);
  });

  /// The CLI asks whether a folder is trusted on its own screen, before it
  /// runs anything -- no hook, no transcript, nothing. The island puts the
  /// question where the person is, and answers it only as they answer it.
  /// tech.md 6.24.
  test('the question about a folder is asked where the person is', async ({ page }) => {
    const asking = { ...aimed('/Users/dev/whitelist'), asking_trust: Date.now() - 2_000 };
    await stub(page, [asking]);
    await page.goto(ROUTE);

    await expect(page.getByText('Claude Code asks whether you trust this folder')).toBeVisible();
    // Which folder, in full: the answer is about this path and no other.
    await expect(page.locator('.trust .where')).toHaveText('/Users/dev/whitelist');

    await page.getByRole('button', { name: 'Trust it' }).click();
    const calls = await page.evaluate(
      () => (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls,
    );
    const answered = calls.filter((call) => call.command === 'answer_trust');
    expect(answered).toHaveLength(1);
    expect(answered[0].args).toEqual({ sessionId: 's1', trust: true });
  });

  /// And no is an answer too: it is the question's own `No, exit`.
  test('the folder question can be answered with no', async ({ page }) => {
    const asking = { ...aimed('/Users/dev/whitelist'), asking_trust: Date.now() - 2_000 };
    await stub(page, [asking]);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Not here' }).click();
    const calls = await page.evaluate(
      () => (window as unknown as { __calls: { command: string; args: unknown }[] }).__calls,
    );
    const answered = calls.filter((call) => call.command === 'answer_trust');
    expect(answered).toHaveLength(1);
    expect(answered[0].args).toEqual({ sessionId: 's1', trust: false });
  });

  /// A chat nobody is asking about carries no panel: it is an answer to a
  /// question, and there is no question.
  test('a chat with nothing to answer carries no panel', async ({ page }) => {
    await stub(page, [aimed('/Users/dev/peekle')]);
    await page.goto(ROUTE);

    await expect(page.locator('.aim')).toBeVisible();
    await expect(page.locator('.trust')).toHaveCount(0);
  });

  /// A screenshot in a message is a reference to the picture, so pressing it
  /// has to open the picture -- it is the only way to see what is on it. It
  /// did not: the rule that takes an open picture away when its attachment is
  /// taken back was written for the row above the field and ran on every open
  /// picture, so one opened out of the feed closed itself in the tick it
  /// opened. tech.md 6.13.
  test('a screenshot in a message opens when it is pressed', async ({ page }) => {
    const SHOT = '/Users/dev/Library/Caches/peekle/shots/01M238H5HQEQB3GY5V1SMPPFYF.png';
    await stub(page, [observed([say('u1', `${SHOT}\nlook at this`, 1_789_000_000_000)])]);
    await page.goto(ROUTE);

    const block = page.getByRole('button', { name: /Open the screenshot/ });
    await expect(block).toBeVisible();
    // What it says about the picture is measured off the picture itself.
    await expect(block).toContainText('1172\u00d7246');

    await block.click();
    await expect(page.locator('.preview img')).toBeVisible();

    // And it closes, the way it always did.
    await page.keyboard.press('Escape');
    await expect(page.locator('.preview')).toHaveCount(0);
    await expect(block).toBeVisible();
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
    // And the ground is under all of it. It was not: with `nowrap` under a
    // ceiling the words ran past the box they were drawn on, and the tail of
    // the line stood on the conversation.
    const fits = await hint.evaluate((node) => ({
      wide: node.scrollWidth <= node.clientWidth + 1,
      tall: node.scrollHeight <= node.clientHeight + 1,
    }));
    expect(fits).toEqual({ wide: true, tall: true });

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
