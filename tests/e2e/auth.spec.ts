/**
 * v83 acceptance, end to end on the island route with a stub where Tauri
 * would be. tech.md 6.16.
 *
 * Rust is played by the stub: commands answer from a fixture, and the events
 * Rust would send -- `peekle://account`, `peekle://sign-in`, `peekle://view`,
 * `peekle://usage` -- are pushed the way Rust pushes them. Every command the
 * route sends is kept on `window.__calls`, because what a press sends is as
 * much the contract as what it looks like.
 */

import { expect, test, type Page } from '@playwright/test';

const ROUTE = '/island/?notch=34&notch_width=185';
const INSTALL = 'curl -fsSL https://claude.ai/install.sh | bash';

type Account = {
  cli: 'Missing' | 'Outdated' | 'Ready';
  version: string | null;
  install: 'Native' | 'Homebrew' | 'Npm' | 'Unknown';
  signed_in: boolean | null;
  command: string | null;
};

const signedOut: Account = {
  cli: 'Ready',
  version: '2.1.263',
  install: 'Homebrew',
  signed_in: false,
  command: null,
};
const signedIn: Account = { ...signedOut, signed_in: true };
const missing: Account = {
  cli: 'Missing',
  version: null,
  install: 'Unknown',
  signed_in: null,
  command: INSTALL,
};
const outdated: Account = {
  cli: 'Outdated',
  version: '2.0.14',
  install: 'Homebrew',
  signed_in: null,
  command: 'brew upgrade --cask claude-code@latest',
};

/** A chat from before: the thing a signed-out person must not be shown. */
const HISTORY_TITLE = 'Refactor the billing module';
const history = {
  session: {
    session_id: 's1',
    cwd: '/Users/dev/peekle',
    project: 'peekle',
    pid: null,
    tty: null,
  },
  title: HISTORY_TITLE,
  status: 'Idle',
  origin: 'Observed',
  entries: [],
  agent: null,
  mode: null,
  thinking: null,
  compacting: null,
  stopping: null,
  asking_trust: null,
  updated_at: 0,
};

function usage(reason: string | null) {
  return {
    windows: reason
      ? []
      : [
          { window: 'FiveHour', used_pct: 12, resets_at: null },
          { window: 'SevenDay', used_pct: 30, resets_at: null },
        ],
    source: reason ? 'Unavailable' : 'Account',
    reason,
    fetched_at: 0,
    keychain_granted: true,
    retry_after_ms: null,
  };
}

async function stub(
  page: Page,
  account: Account,
  usageReason: string | null = null,
  sessions: unknown[] = [history],
) {
  await page.addInitScript(
    ({ account, sessions, snapshot }) => {
      const w = window as unknown as Record<string, unknown>;
      const handlers: Record<string, number> = {};
      const calls: { command: string; args: unknown }[] = [];
      w.__calls = calls;
      let counter = 0;
      let current = account;
      const idle = { stage: 'Idle', url: null, needs_code: false, error: null };

      const answer = (command: string): unknown => {
        switch (command) {
          case 'get_state':
            return {
              enabled: true,
              view: 'Sessions',
              active_prompt: null,
              sessions,
              tasks: [],
              usage: snapshot,
              shot: null,
              live_sessions: 0,
              hotkey_ok: true,
            };
          case 'get_sessions':
            return sessions;
          case 'get_account':
          case 'refresh_account':
            return current;
          case 'refresh_usage':
            return snapshot;
          case 'start_sign_in':
            return { ...idle, stage: 'Starting' };
          case 'submit_sign_in_code':
            return { ...idle, stage: 'Finishing' };
          case 'copy_account_command':
            return current.command;
          case 'sign_out':
            current = { ...current, signed_in: false };
            return current;
          case 'get_models':
            return [];
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

      const push = (event: string, payload: unknown) => {
        const handler = w[`_${handlers[event]}`] as ((message: unknown) => void) | undefined;
        handler?.({ event, id: 1, payload });
      };
      w.__account = (next: typeof account) => {
        current = next;
        push('peekle://account', next);
      };
      w.__signIn = (next: unknown) => push('peekle://sign-in', next);
      w.__view = (next: unknown) => push('peekle://view', next);
    },
    { account, sessions, snapshot: usage(usageReason) },
  );
}

async function push(page: Page, fn: '__account' | '__signIn' | '__view', payload: unknown) {
  await page.evaluate(
    ([name, value]) => (window as unknown as Record<string, (v: unknown) => void>)[name](value),
    [fn, payload] as const,
  );
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

const run = (stage: string, over: Record<string, unknown> = {}) => ({
  stage,
  url: null,
  needs_code: false,
  error: null,
  ...over,
});

test.describe('the sign-in window', () => {
  /// The first run: no sessions at all. An effect that read and wrote the
  /// same list state looped on an empty list, Svelte stopped every effect on
  /// the page, and the island froze at the size of the resting mark.
  test('opens in full on an empty list, with no error on the page', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await stub(page, signedIn, null, []);
    await page.goto(ROUTE);

    await expect(page.getByText('No sessions yet', { exact: false })).toBeVisible();
    await expect
      .poll(async () => (await page.locator('.shape').boundingBox())?.width ?? 0)
      .toBeGreaterThan(400);
    expect(errors).toEqual([]);
  });

  /// And the path a new person actually takes: signed out, nothing in the
  /// list, the browser approves, and the empty list stands open.
  test('signs a new person in and lands on their empty list', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await stub(page, signedOut, null, []);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Sign in with Claude' }).click();
    await push(page, '__signIn', run('Waiting', { url: 'https://claude.com/cai/oauth/authorize' }));
    await expect(page.getByRole('heading', { name: 'Continue in your browser' })).toBeVisible();
    await push(page, '__account', signedIn);
    await push(page, '__signIn', run('Done'));
    await expect(page.getByRole('heading', { name: "You're signed in" })).toBeVisible();

    await expect(page.getByText('No sessions yet', { exact: false })).toBeVisible({
      timeout: 4000,
    });
    expect(errors).toEqual([]);
  });

  /// The owner's words: a person who is not signed in sees the window and
  /// nothing of their past sessions.
  test('stands alone while signed out: no history, no search, no new session', async ({ page }) => {
    await stub(page, signedOut);
    await page.goto(ROUTE);

    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toBeVisible();
    await expect(page.getByText(HISTORY_TITLE)).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'New session' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Settings' })).toHaveCount(0);
  });

  /// The whole point of v83: Allow in the browser finishes the sign-in with
  /// nothing pressed in the island, and the list comes back by itself.
  test('picks up an approval from the browser and gives the island back', async ({ page }) => {
    await stub(page, signedOut);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Sign in with Claude' }).click();
    expect(await calls(page, 'start_sign_in')).toHaveLength(1);
    await expect(page.getByRole('heading', { name: 'Opening your browser' })).toBeVisible();

    await push(
      page,
      '__signIn',
      run('Waiting', { url: 'https://claude.com/cai/oauth/authorize', needs_code: true }),
    );
    await expect(page.getByRole('heading', { name: 'Continue in your browser' })).toBeVisible();
    // No code is asked for: the browser answers on its own.
    await expect(page.getByLabel('Paste the code')).toHaveCount(0);

    // What Rust sends when the CLI exits after the browser came back.
    await push(page, '__signIn', run('Finishing'));
    await expect(page.getByRole('status', { name: 'Signing in' })).toBeVisible();
    await push(page, '__account', signedIn);
    await push(page, '__signIn', run('Done'));
    await expect(page.getByRole('heading', { name: "You're signed in" })).toBeVisible();

    // The tick stands a moment, then the island is the island again.
    await expect(page.getByText(HISTORY_TITLE)).toBeVisible({ timeout: 4000 });
    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toHaveCount(0);
    expect(await calls(page, 'submit_sign_in_code')).toHaveLength(0);
  });

  test('says a refusal was a refusal, and starts again on the press', async ({ page }) => {
    await stub(page, signedOut);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Sign in with Claude' }).click();
    await push(page, '__signIn', run('Denied'));

    await expect(page.getByRole('heading', { name: 'Access declined' })).toBeVisible();
    await expect(page.getByText(HISTORY_TITLE)).toHaveCount(0);
    await page.getByRole('button', { name: 'Try again' }).click();
    expect(await calls(page, 'start_sign_in')).toHaveLength(2);
  });

  /// The page showed a code rather than coming back: the code goes in here.
  test('takes the code the page showed', async ({ page }) => {
    await stub(page, signedOut);
    await page.goto(ROUTE);

    await page.getByRole('button', { name: 'Sign in with Claude' }).click();
    await push(
      page,
      '__signIn',
      run('Waiting', { url: 'https://claude.com/cai/oauth/authorize', needs_code: true }),
    );
    // The page with the code can be opened again from here while it waits.
    await page.getByRole('button', { name: 'Open the page again' }).click();
    expect(await calls(page, 'open_sign_in_page')).toHaveLength(1);

    await page.getByRole('button', { name: 'Have a code?' }).click();
    await page.getByLabel('Paste the code').fill('BzU6xWpt#KlHBCCN7kR9');
    await page.getByLabel('Paste the code').press('Enter');

    const sent = await calls(page, 'submit_sign_in_code');
    expect(sent).toEqual([
      { command: 'submit_sign_in_code', args: { code: 'BzU6xWpt#KlHBCCN7kR9' } },
    ]);
    // The code went in: the window says it is signing in, and waits for the
    // exit to give the verdict.
    await expect(page.getByRole('status', { name: 'Signing in' })).toBeVisible();
  });

  test('asks to install Claude Code, copies the command, and links the guide', async ({ page }) => {
    await stub(page, missing);
    await page.goto(ROUTE);

    await expect(page.getByRole('heading', { name: 'Install Claude Code' })).toBeVisible();
    await expect(page.getByText(INSTALL)).toBeVisible();
    await expect(page.getByRole('button', { name: 'Sign in with Claude' })).toHaveCount(0);

    await page.getByRole('button', { name: 'Copy' }).click();
    expect(await calls(page, 'copy_account_command')).toHaveLength(1);
    await expect(page.getByRole('button', { name: 'Copied' })).toBeVisible();

    await page.getByRole('button', { name: 'Installation guide' }).click();
    expect(await calls(page, 'open_account_link')).toEqual([
      { command: 'open_account_link', args: { link: 'InstallGuide' } },
    ]);

    // Installed in a terminal: the next answer changes the window by itself.
    await push(page, '__account', signedOut);
    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toBeVisible();
  });

  test('asks to update an old Claude Code, with its version and both links', async ({ page }) => {
    await stub(page, outdated);
    await page.goto(ROUTE);

    await expect(page.getByRole('heading', { name: 'Update Claude Code' })).toBeVisible();
    await expect(
      page.getByText('Claude Code 2.0.14 is too old to sign in from Peekle.'),
    ).toBeVisible();
    await expect(page.getByText('brew upgrade --cask claude-code@latest')).toBeVisible();

    await page.getByRole('button', { name: "What's new" }).click();
    await page.getByRole('button', { name: 'Update guide' }).click();
    expect((await calls(page, 'open_account_link')).map((call) => call.args)).toEqual([
      { link: 'Changelog' },
      { link: 'UpdateGuide' },
    ]);

    await page.getByRole('button', { name: 'Check again' }).click();
    await expect.poll(async () => (await calls(page, 'refresh_account')).length).toBeGreaterThan(0);
  });

  /// v83.1: out of the settings and straight into the sign-in window, with a
  /// second press standing between, because the terminal signs out too.
  test('signs out from the settings and shows the sign-in window', async ({ page }) => {
    await stub(page, signedIn);
    await page.goto(ROUTE);
    await expect(page.getByText(HISTORY_TITLE)).toBeVisible();

    await page.getByRole('button', { name: 'Settings' }).click();
    await page.getByRole('button', { name: 'Sign out' }).click();
    await expect(
      page.getByText('Signs Claude Code out on this Mac, the terminal included.'),
    ).toBeVisible();
    expect(await calls(page, 'sign_out')).toHaveLength(0);

    await page.getByRole('button', { name: 'Sign out' }).click();
    expect(await calls(page, 'sign_out')).toHaveLength(1);
    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toBeVisible();
    await expect(page.getByText(HISTORY_TITLE)).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Settings' })).toHaveCount(0);
  });

  test('is not there for a signed-in account', async ({ page }) => {
    await stub(page, signedIn);
    await page.goto(ROUTE);

    await expect(page.getByText(HISTORY_TITLE)).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toHaveCount(0);
  });

  /// The regression of the screenshots: Claude Code signed in, the usage
  /// endpoint refusing, and a screen with no way out in front of everything.
  test('never shows the refused dead end over a signed-in account', async ({ page }) => {
    await stub(page, signedIn, 'NotLoggedIn');
    await page.goto(ROUTE);

    await expect(page.getByText(HISTORY_TITLE)).toBeVisible();
    await expect(page.getByText(/API refused/)).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toHaveCount(0);

    await page.getByText(HISTORY_TITLE).click();
    await expect(page.getByText(/API refused/)).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'Sign in to Peekle' })).toHaveCount(0);
  });
});
