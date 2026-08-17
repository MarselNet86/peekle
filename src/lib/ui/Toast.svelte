<script lang="ts">
  import { fly } from 'svelte/transition';
  import type { ToastTone } from '$lib/types/generated/ToastTone';

  let {
    text,
    tone = 'Neutral',
    badge,
  }: { text: string; tone?: ToastTone; badge?: number | null } = $props();
</script>

<div
  class="toast"
  data-tone={tone}
  in:fly={{ y: -12, duration: 220 }}
  out:fly={{ y: -12, duration: 160 }}
>
  <span class="mark"></span>
  <span class="text">{text}</span>
  {#if badge != null}
    <span class="badge">{badge}</span>
  {/if}
</div>

<style>
  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 100%;
    padding: 0 16px;
    background: var(--surface-hud);
    backdrop-filter: blur(var(--blur)) saturate(140%);
    border: 1px solid var(--hairline);
    border-radius: 999px;
    color: var(--text);
    font-size: 13px;
  }

  .mark {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  .toast[data-tone='On'] .mark {
    background: var(--accent);
  }
  .toast[data-tone='Off'] .mark {
    background: var(--text-dim);
  }
  .toast[data-tone='Warn'] .mark {
    background: var(--warn);
  }

  .text {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
  }
</style>
