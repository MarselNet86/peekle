<script lang="ts">
  import type { RestStatus } from '$lib/logic/rest';
  import { REST_SIDE } from '$lib/logic/shape';
  import { clampPct, usageTone } from '$lib/logic/usage';

  let { status, pct, onopen }: { status: RestStatus; pct: number | null; onopen: () => void } =
    $props();

  /** A 4 by 6 grid of pixels. Cells lit for the letter p. */
  const PIXELS = [
    [0, 0],
    [1, 0],
    [2, 0],
    [0, 1],
    [3, 1],
    [0, 2],
    [3, 2],
    [0, 3],
    [1, 3],
    [2, 3],
    [0, 4],
    [0, 5],
  ];

  const RADIUS = 6;
  const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

  const known = $derived(pct !== null);
  const value = $derived(known ? clampPct(pct as number) : 0);
  const tone = $derived(usageTone(value));
  // The ring runs anticlockwise from twelve o'clock, so a full window reads as
  // a closed circle rather than as an arc that stopped somewhere.
  const dash = $derived(`${(value / 100) * CIRCUMFERENCE} ${CIRCUMFERENCE}`);

  const labels: Record<RestStatus, string> = {
    idle: 'Peekle is running',
    working: 'Claude is working',
    waiting: 'Claude is waiting on you',
  };
  const usageLabel = $derived(known ? `, ${Math.round(value)}% of the 5h window used` : '');
</script>

<!-- The whole resting shape is the target. Its middle is behind the camera
     housing, so asking the user to hit either end would be unfair. tech.md 6.7. -->
<button
  class="mark"
  data-status={status}
  style:--side="{REST_SIDE}px"
  aria-label="{labels[status]}{usageLabel}. Open the session list"
  onclick={onopen}
>
  <span class="glyph">
    <svg viewBox="0 0 4 6" width="8" height="12" aria-hidden="true">
      {#each PIXELS as [x, y] (`${x}:${y}`)}
        <rect {x} {y} width="1" height="1" rx="0.28" fill="currentColor" />
      {/each}
    </svg>
  </span>

  <span class="ring" data-known={known}>
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <circle cx="8" cy="8" r={RADIUS} fill="none" stroke="var(--hairline)" stroke-width="2" />
      {#if known}
        <circle
          cx="8"
          cy="8"
          r={RADIUS}
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-dasharray={dash}
          transform="rotate(-90 8 8)"
          data-tone={tone}
        />
      {/if}
    </svg>
  </span>
</button>

<style>
  .mark {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 100%;
    /* Both ends live in the overhang beside the cutout, which is the only part
       of a resting island that has pixels at all. tech.md 6.7. */
    padding: 0 calc((var(--side) - 16px) / 2);
    border: none;
    background: transparent;
    cursor: pointer;
  }

  .glyph {
    display: flex;
    color: var(--text-dim);
    /* Colour only. The shape underneath belongs to Shape and never moves for a
       hover. tech.md 6.10. */
    transition: color 160ms ease-out;
  }

  .mark[data-status='working'] .glyph {
    color: var(--text);
  }

  /* Waiting is the one state that costs the user time, so it is the one state
     that moves. Opacity only: filter and backdrop-filter repaint everything
     under the window on every frame. tech.md 6.10. */
  .mark[data-status='waiting'] .glyph {
    color: var(--accent);
    animation: breathe 1600ms ease-in-out infinite;
  }

  .mark:hover .glyph {
    color: var(--text);
  }

  .ring {
    display: flex;
    color: var(--accent);
  }

  /* The same four thresholds UsageBar draws, because two readouts of one
     number that disagree are worse than one of them missing. tech.md 9. */
  .ring:has([data-tone='warn']) {
    color: var(--warn);
  }
  .ring:has([data-tone='orange']) {
    color: var(--orange);
  }
  .ring:has([data-tone='danger']) {
    color: var(--danger);
  }

  @keyframes breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.45;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mark[data-status='waiting'] .glyph {
      animation: none;
    }
  }
</style>
