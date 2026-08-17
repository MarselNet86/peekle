<script lang="ts">
  import { fly } from 'svelte/transition';
  import type { ToastTone } from '$lib/types/generated/ToastTone';

  let {
    text,
    tone = 'Neutral',
    badge,
  }: { text: string; tone?: ToastTone; badge?: number | null } = $props();
</script>

<!-- Not a pill floating under the notch: the same black, flush with the top
     edge, so the notch reads as having grown. tech.md 6.7 and 9. -->
<div
  class="notch"
  data-tone={tone}
  in:fly={{ y: -14, duration: 260, opacity: 1 }}
  out:fly={{ y: -14, duration: 180, opacity: 1 }}
>
  <div class="band">
    <span class="mark"></span>
    <span class="text">{text}</span>
    {#if badge != null}
      <span class="badge">{badge}</span>
    {/if}
  </div>
</div>

<style>
  .notch {
    width: 100%;
    height: 100%;
    background: var(--notch);
    /* Square at the top because the screen edge cuts it, rounded below so it
       matches the curve the bezel already has. */
    border-radius: 0 0 20px 20px;
    /* Content clears the camera housing. Zero on a display without one. */
    padding-top: var(--notch-h, 0px);
    box-sizing: border-box;
    color: var(--text);
  }

  /* No notch to continue, so it goes back to being a floating pill. */
  :global(:root:not([data-notch])) .notch {
    border-radius: 999px;
  }

  .band {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding: 0 18px;
  }

  .mark {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  .notch[data-tone='On'] .mark {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }
  .notch[data-tone='Off'] .mark {
    background: var(--text-dim);
  }
  .notch[data-tone='Warn'] .mark {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
  }
  .notch[data-tone='Neutral'] .mark {
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
