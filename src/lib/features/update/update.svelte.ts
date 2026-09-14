/**
 * What the island knows about a new version. Copies the island layout: bridge
 * in, runes out, Rust owning the fact. tech.md 6.30.
 *
 * The view is Rust's here as everywhere else. Nothing in this file opens the
 * island: the check raises `IslandView::Update` on its own once the file is on
 * disk, and this only renders what is already decided.
 */

import { commands, events } from '$lib/bridge';
import { offered } from '$lib/logic/update';
import type { UpdateState } from '$lib/types/generated/UpdateState';

export function createUpdate() {
  let state = $state<UpdateState>('Idle');
  // Install was pressed: the buttons lock and the panel fades while Rust hands
  // the file to Finder and stands aside.
  let busy = $state(false);

  async function start(): Promise<() => void> {
    const off = await events.onUpdate((next) => {
      state = next;
      // A state that is not an offer cannot be the one being installed: a
      // second check while the panel stood would otherwise leave the buttons
      // locked with nothing to unlock them.
      if (!offered(next)) busy = false;
    });
    return off;
  }

  function install() {
    if (busy || !offered(state)) return;
    busy = true;
    // A refusal leaves the panel up rather than folding it: the file is gone or
    // the pasteboard would not take the command, and there is nothing to fold
    // on. Rust says so out loud in both cases.
    commands.installUpdate()?.catch(() => {
      busy = false;
    });
  }

  function later() {
    if (busy) return;
    commands.dismissUpdate();
  }

  function notes() {
    const update = offered(state);
    if (update?.notes_url) commands.openUpdateNotes();
  }

  return {
    get state() {
      return state;
    },
    /** The update on offer, or null in every other state. */
    get update() {
      return offered(state);
    },
    get busy() {
      return busy;
    },
    start,
    install,
    later,
    notes,
  };
}
