/**
 * Signing in from the island. tech.md 6.16.
 *
 * Everything real happens in `claude auth login`, driven by Rust in a pty.
 * This holds what the panel needs to draw: how far it has got, the authorize
 * address in case the browser did not open, and whether a press is in flight.
 */

import { commands, events } from '$lib/bridge';
import type { SignInState } from '$lib/types/generated/SignInState';
import type { SignInStage } from '$lib/types/generated/SignInStage';

const IDLE: SignInState = { stage: 'Idle', url: null, needs_code: false, error: null };

/** The stages where a process is up and the panel must stay open. */
const OPEN: SignInStage[] = ['Starting', 'Waiting', 'Finishing'];

export function isOpen(state: SignInState): boolean {
  return OPEN.includes(state.stage);
}

/**
 * What the panel says while it waits, in the order the CLI does things.
 *
 * Every stage says something. A panel that goes blank between the press and
 * the address reads as a press that was lost, which is the fault this whole
 * path exists to fix.
 */
export function waitingText(state: SignInState): string {
  switch (state.stage) {
    case 'Starting':
      return 'Starting Claude Code';
    case 'Waiting':
      return state.needs_code
        ? 'Approve in your browser, then paste the code'
        : 'Opening your browser';
    case 'Finishing':
      return 'Signing in';
    default:
      return '';
  }
}

export function createSignIn() {
  let state = $state<SignInState>(IDLE);
  let busy = $state(false);

  async function start(): Promise<() => void> {
    return await events.onSignIn((next) => {
      state = next;
      // Rust has moved on, so nothing here is still in flight. Without this a
      // failure that came back as an event would leave the button spinning
      // over a process that is already gone.
      busy = false;
    });
  }

  /** The press. Rust decides whether a login is even the right answer. */
  async function begin() {
    busy = true;
    try {
      const next = await commands.startSignIn();
      if (next) state = next;
    } finally {
      busy = false;
    }
  }

  /** The code from the authorize page. Empty is not a submission. */
  async function submit(code: string) {
    if (!code.trim()) return;
    busy = true;
    try {
      const next = await commands.submitSignInCode(code);
      if (next) state = next;
    } finally {
      busy = false;
    }
  }

  /** The way back when Claude Code's own `open` did not land. 6.16. */
  async function openPage() {
    await commands.openSignInPage();
  }

  async function cancel() {
    // Locally first: the panel must close on the press, not a round trip
    // later. Rust settles the process, and its event confirms the same thing.
    state = IDLE;
    busy = false;
    await commands.cancelSignIn();
  }

  return {
    get state() {
      return state;
    },
    get open() {
      return isOpen(state);
    },
    get busy() {
      return busy;
    },
    get waitingText() {
      return waitingText(state);
    },
    begin,
    submit,
    openPage,
    cancel,
    start,
  };
}
