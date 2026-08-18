<script lang="ts">
  import type { SessionCard } from '$lib/types/generated/SessionCard';

  let { card, onopen }: { card: SessionCard; onopen?: () => void } = $props();

  /** One line of status, in the words a person would use. */
  const STATUS: Record<string, string> = {
    Working: 'working',
    WaitingOnUser: 'waiting on you',
    Idle: 'idle',
    Ended: 'ended',
  };
</script>

<button class="row" data-status={card.status} onclick={() => onopen?.()}>
  <span class="dot"></span>
  <span class="body">
    <span class="title">{card.title || card.session.project}</span>
    <span class="status">{card.session.project} · {STATUS[card.status] ?? card.status}</span>
  </span>
  <svg class="chevron" viewBox="0 0 8 12" width="8" height="12" aria-hidden="true">
    <path d="M1.5 1l5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
  </svg>
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: calc(var(--row) + 10px);
    padding: 0 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .row:hover {
    background: rgba(255, 255, 255, 0.06);
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
