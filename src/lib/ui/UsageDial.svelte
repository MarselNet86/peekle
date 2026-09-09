<script lang="ts">
  import { clampPct, usageTone } from '$lib/logic/usage';

  let {
    pct,
    label,
    size = 15,
    track = true,
  }: {
    pct: number | null;
    label?: string;
    size?: number;
    /** The grey circle under the arc. It says how much of the window is still
     * whole, which is worth drawing where the window is the subject. Where
     * the ring is a button (6.15) it is not: a bright circle at the edge of
     * the row pulls the eye and reports nothing. */
    track?: boolean;
  } = $props();

  const known = $derived(pct !== null);
  const value = $derived(known ? clampPct(pct as number) : 0);
  const tone = $derived(usageTone(value));
</script>

<!-- Never call this a `ring`: Tailwind owns that class name and paints its own
     box-shadow over anything wearing it. tech.md 9. -->
<span
  class="usage-dial"
  title={known ? `${Math.round(value)}% of ${label ?? 'the window'}` : label}
>
  <span
    class="dial"
    data-tone={known ? tone : undefined}
    style:--pct={value}
    style:width="{size}px"
    style:height="{size}px"
  >
    <!-- Held while there is no number, whatever the caller asked for: with
         no arc and no track there is nothing on screen at all, and a button
         that is not there cannot be pressed. -->
    {#if track || !known}
      <span class="track"></span>
    {/if}
    {#if known}
      <span class="fill"></span>
    {/if}
    <span class="hole"></span>
  </span>
  {#if label}
    <span class="label">{known ? `${Math.round(value)}%` : '––'} {label}</span>
  {/if}
</span>

<style>
  .usage-dial {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .dial {
    position: relative;
    flex: none;
    color: var(--accent);
  }

  /* The same four thresholds UsageBar draws, because two readouts of one
     number that disagree are worse than one of them missing. tech.md 9. */
  .dial[data-tone='warn'] {
    color: var(--warn);
  }
  .dial[data-tone='orange'] {
    color: var(--orange);
  }
  .dial[data-tone='danger'] {
    color: var(--danger);
  }

  .track,
  .fill,
  .hole {
    position: absolute;
    border-radius: 50%;
  }

  /* Dimmed rather than hairline: that token is meant for one pixel separators
     and disappears at two pixels on pure black. */
  .track {
    inset: 0;
    box-sizing: border-box;
    border: 2px solid var(--text-dim);
    opacity: 0.3;
  }

  /* The arc is a full disc cut back by the hole rather than a mask: masks are
     the one part of this the app webview renders differently. */
  .fill {
    inset: 0;
    background: conic-gradient(currentColor calc(var(--pct) * 1%), transparent 0);
  }

  /* The island fill is exactly black and fully opaque (tech.md 9), so punching
     the middle out with it is the same thing as punching a hole. */
  .hole {
    inset: 2px;
    background: var(--notch);
  }

  .label {
    font-size: 11px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
