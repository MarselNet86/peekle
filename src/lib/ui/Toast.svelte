<script lang="ts">
  import type { ToastTone } from '$lib/types/generated/ToastTone';

  let {
    text,
    tone = 'Neutral',
    badge,
  }: { text: string; tone?: ToastTone; badge?: number | null } = $props();
</script>

<!-- Content of the pill only. The black fill, the corners and the movement
     belong to Shape. tech.md 9. -->
<div class="band" data-tone={tone}>
  <span class="mark"></span>
  <span class="text">{text}</span>
  {#if badge != null}
    <span class="badge">{badge}</span>
  {/if}
</div>

<style>
  .band {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding: 0 18px;
    box-sizing: border-box;
    color: var(--text);
  }

  .mark {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  .band[data-tone='On'] .mark {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }
  .band[data-tone='Off'] .mark {
    background: var(--text-dim);
  }
  .band[data-tone='Warn'] .mark {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
  }
  .band[data-tone='Neutral'] .mark {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }

  .text {
    flex: 1;
    font-size: 13px;
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    flex: none;
    min-width: 18px;
    padding: 1px 6px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
</style>
