/**
 * Island state. The reference vertical every later slice copies: bridge in,
 * runes hold what the window renders, Rust decides what the island shows.
 *
 * The view is Rust's, not the frontend's. Nothing here sets it directly; it
 * arrives on `peekle://view` and leaves as a `set_view` intent. tech.md 8.
 */

import { commands, events } from '$lib/bridge';
import { readNotch, type Notch } from '$lib/logic/shape';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';
import type { ToastRequest } from '$lib/types/generated/ToastRequest';

export function createIsland(search = '') {
  // The query string carries the first frame. After that the notch arrives as
  // an event, because the island can open on a different display than it did
  // last time and reloading the route would throw away the feed and the reply
  // being typed. tech.md 6.7.
  let notch = $state<Notch>(readNotch(search));

  let view = $state<IslandView>('Collapsed');
  let toast = $state<ToastRequest | null>(null);
  let prompt = $state<PromptRequest | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(next: ToastRequest) {
    clearTimeout(timer);
    toast = next;
    // Rust collapses the island on the same deadline. Clearing here too keeps
    // the last toast from flashing back when the next one arrives.
    timer = setTimeout(() => {
      toast = null;
    }, next.ttl_ms);
  }

  /**
   * Typed text goes into the session's pty and leaves at once. tech.md 6.5.
   *
   * A permission request open on screen takes precedence, because there the
   * text is the reason for a denial rather than a message.
   */
  function answer(text: string, sessionId?: string) {
    const trimmed = text.trim();
    if (!trimmed) return;

    const open = prompt;
    if (open) {
      prompt = null;
      commands.answerPrompt({ prompt_id: open.id, choice: null, text: trimmed });
      return;
    }
    if (sessionId) commands.sendMessage(sessionId, trimmed);
  }

  function choose(choiceId: string) {
    const open = prompt;
    if (!open) return;
    prompt = null;
    commands.answerPrompt({ prompt_id: open.id, choice: choiceId, text: null });
  }

  function dismiss() {
    const open = prompt;
    if (!open) return;
    prompt = null;
    commands.dismissPrompt(open.id);
  }

  async function start(): Promise<() => void> {
    const [offToast, offView, offNotch, offOpen, offClose] = await Promise.all([
      events.onToast(show),
      events.onView((next) => {
        view = next;
      }),
      events.onNotch((next) => {
        // A display with no notch reports zero, which is the floating pill.
        notch = { width: next.width > 0 ? next.width : notch.width, height: next.height };
      }),
      events.onPromptOpen((request) => {
        prompt = request;
      }),
      // Rust settles a request on timeout and on bypass too, so the field has
      // to clear on an outcome the user never chose.
      events.onPromptClose(({ prompt_id }) => {
        if (prompt?.id === prompt_id) prompt = null;
      }),
    ]);

    const state = await commands.getState();
    // A late mount must not drop what Rust already decided on.
    if (state) {
      view = state.view;
      prompt = state.active_prompt;
    }

    return () => {
      clearTimeout(timer);
      offToast();
      offView();
      offNotch();
      offOpen();
      offClose();
    };
  }

  return {
    get notch() {
      return notch;
    },
    get view() {
      return view;
    },
    get toast() {
      return toast;
    },
    get prompt() {
      return prompt;
    },
    answer,
    choose,
    dismiss,
    show,
    start,
  };
}
