<script lang="ts">
  import { ageLabel } from '$lib/logic/age';
  import type { SessionCard } from '$lib/types/generated/SessionCard';

  let {
    card,
    now = Date.now(),
    onopen,
    onrename,
    ondelete,
    fault = null,
  }: {
    card: SessionCard;
    now?: number;
    onopen?: () => void;
    onrename?: (title: string) => void;
    /** Delete the chat: its process, its transcript and this row. Asked twice
     * by the button itself. tech.md 6.26. */
    ondelete?: () => void;
    /** Why the last deletion did not happen. The row stays and says so: a
     * press that changes nothing and explains nothing reads as a dead
     * button. tech.md 9. */
    fault?: string | null;
  } = $props();

  let editing = $state(false);
  let draft = $state('');
  let field: HTMLInputElement | undefined = $state();

  /** How long the question stands before the row goes back to itself. Long
   * enough to move the hand to the same button, short enough that a row left
   * alone is never found asking. tech.md 6.26. */
  const CONFIRM_FOR = 4000;

  let asking = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const age = $derived(ageLabel(card.updated_at, now));

  function forget() {
    clearTimeout(timer);
    asking = false;
  }

  /**
   * The trash asks with itself rather than with a dialog: the island has one
   * shape (6.7), and a modal window for one row would be a second one. Two
   * presses, because the first one ends a live process, and a slip of the
   * mouse costs a turn. tech.md 6.26.
   */
  function press() {
    if (asking) {
      forget();
      ondelete?.();
      return;
    }
    asking = true;
    clearTimeout(timer);
    timer = setTimeout(() => (asking = false), CONFIRM_FOR);
  }

  // The question belongs to the press, not to the row: a hand that went
  // somewhere else has answered it. tech.md 6.26.
  $effect(() => () => clearTimeout(timer));

  function edit() {
    forget();
    draft = card.title;
    editing = true;
  }

  // Focus only here, because only here did the user ask for the keyboard.
  // tech.md 6.7.
  $effect(() => {
    if (editing) field?.focus();
  });

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      editing = false;
      onrename?.(draft);
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      editing = false;
    }
  }

  /** One line of status, in the words a person would use. */
  const STATUS: Record<string, string> = {
    Working: 'working',
    WaitingOnUser: 'waiting on you',
    Idle: 'idle',
    Ended: 'ended',
  };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="row"
  data-status={card.status}
  data-editing={editing}
  class:asking
  onpointerleave={forget}
>
  <span class="dot"></span>

  {#if editing}
    <!-- Enter keeps it, Escape leaves the name alone. -->
    <input
      class="rename"
      bind:this={field}
      bind:value={draft}
      onkeydown={keydown}
      onblur={() => (editing = false)}
      aria-label="Rename this session"
      spellcheck="false"
    />
  {:else}
    <button class="open" onclick={() => onopen?.()}>
      <span class="body">
        <span class="title">{card.title || card.session.project}</span>
        <!-- What the row says about itself, unless something is about to
             happen to it or something already failed to. tech.md 6.26. -->
        {#if fault}
          <span class="status bad">{fault}</span>
        {:else if asking}
          <span class="status warn">Delete this chat and its transcript?</span>
        {:else}
          <span class="status">{card.session.project} · {STATUS[card.status] ?? card.status}</span>
        {/if}
      </span>
    </button>

    <span class="age">{age}</span>
    <span class="tools">
      <button class="tool" onclick={edit} aria-label="Rename this session">
        <svg viewBox="0 0 14 14" width="12" height="12" aria-hidden="true">
          <path
            d="M9.4 1.8l2.8 2.8L4.9 12H2.1V9.2z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.3"
            stroke-linejoin="round"
          />
        </svg>
      </button>
      <button
        class="tool"
        class:armed={asking}
        onclick={press}
        aria-label={asking ? 'Delete this chat for good' : 'Delete this chat'}
      >
        <svg viewBox="0 0 14 14" width="12" height="12" aria-hidden="true">
          <path
            d="M2.6 3.8h8.8M5.4 3.8V2.4h3.2v1.4M3.8 3.8l.6 8h5.2l.6-8"
            fill="none"
            stroke="currentColor"
            stroke-width="1.3"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>
    </span>
    <svg class="chevron" viewBox="0 0 8 12" width="8" height="12" aria-hidden="true">
      <path d="M1.5 1l5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
    </svg>
  {/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: calc(var(--row) + 10px);
    padding: 0 10px;
    border-radius: 8px;
    color: var(--text);
  }

  .row:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .open {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
    height: 100%;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .rename {
    flex: 1;
    min-width: 0;
    border: none;
    border-bottom: 1px solid var(--brand);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    padding: 2px 0;
  }

  .age {
    flex: none;
    font-size: 11px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  /* The tools appear on the row the pointer is on, the way a picker does it:
     two icons on every row at rest is a list of icons, not a list of sessions. */
  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    opacity: 0;
    transition: opacity 120ms ease-out;
  }

  .row:hover .tools,
  .tools:focus-within {
    opacity: 1;
  }

  .tool {
    display: flex;
    padding: 3px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
  }

  .tool:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.08);
  }

  /* Asking. It stays lit while the question stands, because the answer is a
     second press on this very button and it has to be findable without
     hunting. tech.md 6.26. */
  .tool.armed,
  .tool.armed:hover {
    color: var(--notch);
    background: var(--danger);
  }

  /* A row that is asking keeps its tools out, or the question would vanish
     the moment the pointer left the button for the button. */
  .row.asking .tools {
    opacity: 1;
  }

  .status.warn {
    color: var(--danger);
  }

  .status.bad {
    color: var(--danger);
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  .open:focus,
  .open:focus-visible,
  .tool:focus,
  .tool:focus-visible,
  .rename:focus,
  .rename:focus-visible {
    outline: none;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  /* Waiting is the one that wants the user right now, so it is the loud one. */
  .row[data-status='WaitingOnUser'] .dot {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
  }
  .row[data-status='Working'] .dot {
    background: var(--accent);
  }
  .row[data-status='Ended'] .dot {
    background: transparent;
    border: 1px solid var(--text-dim);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }

  .title {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status {
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chevron {
    flex: none;
    color: var(--text-dim);
  }
</style>
