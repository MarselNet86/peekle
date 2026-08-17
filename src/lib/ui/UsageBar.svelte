<script lang="ts">
  import { clampPct, resetCountdown, usageTone } from '$lib/logic/usage';

  let {
    label,
    pct,
    resetsAt = null,
    reason,
    now = Math.floor(Date.now() / 1000),
  }: {
    label: string;
    pct: number | null;
    resetsAt?: number | null;
    reason?: string;
    now?: number;
  } = $props();

  // A null percentage renders as dashes with the reason spelled out. Numbers
  // are never invented: the headers behind them are undocumented.
  let known = $derived(pct !== null);
  let value = $derived(known ? clampPct(pct as number) : 0);
  let tone = $derived(usageTone(value));
  let countdown = $derived(known ? resetCountdown(resetsAt, now) : (reason ?? ''));
</script>

<div class="bar">
  <span class="label">{label}</span>
  <span class="track">
    {#if known}
      <span class="fill" data-tone={tone} style:width="{value}%"></span>
    {/if}
  </span>
  <span class="value">{known ? `${Math.round(value)}%` : '––'}</span>
  <span class="note">{countdown}</span>
</div>

<style>
  .bar {
    display: grid;
    grid-template-columns: 34px 1fr 34px;
    grid-template-areas: 'label track value' '. note note';
    align-items: center;
    gap: 4px 8px;
    font-size: 11px;
    color: var(--text-dim);
  }

  .label {
    grid-area: label;
  }

  .track {
    grid-area: track;
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }

  .fill {
    display: block;
    height: 100%;
    border-radius: 2px;
  }

  .fill[data-tone='accent'] {
    background: var(--accent);
  }
  .fill[data-tone='warn'] {
    background: var(--warn);
  }
  .fill[data-tone='orange'] {
    background: var(--orange);
  }
  .fill[data-tone='danger'] {
    background: var(--danger);
  }

  .value {
    grid-area: value;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }

  .note {
    grid-area: note;
    font-size: 10px;
  }
</style>
