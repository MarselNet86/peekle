<script lang="ts">
  import { ageLabel } from '$lib/logic/age';
  import type { SessionCard } from '$lib/types/generated/SessionCard';

  let {
    card,
    now = Date.now(),
    onopen,
    onrename,
    onhide,
  }: {
    card: SessionCard;
    now?: number;
    onopen?: () => void;
    onrename?: (title: string) => void;
    onhide?: () => void;
  } = $props();

  let editing = $state(false);
  let draft = $state('');
  let field: HTMLInputElement | undefined = $state();

  const age = $derived(ageLabel(card.updated_at, now));

  function edit() {
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

<div class="row" data-status={card.status} data-editing={editing}>
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
        <span class="status">{card.session.project} · {STATUS[card.status] ?? card.status}</span>
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
      <button class="tool" onclick={() => onhide?.()} aria-label="Remove this session">
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
