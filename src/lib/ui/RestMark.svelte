<script lang="ts">
  import type { RestStatus } from '$lib/logic/rest';
  import { REST_SIDE } from '$lib/logic/shape';
  import { clampPct, usageTone } from '$lib/logic/usage';

  let { status, pct, onopen }: { status: RestStatus; pct: number | null; onopen: () => void } =
    $props();

  const known = $derived(pct !== null);
  const value = $derived(known ? clampPct(pct as number) : 0);
  const tone = $derived(usageTone(value));
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
    <svg viewBox="0 0 14 12" width="14" height="12" aria-hidden="true">
      <path
        d="M4 10.6L6.9 1.4M9.1 10.6L12 1.4"
        stroke="currentColor"
        stroke-width="2.2"
        stroke-linecap="round"
      />
    </svg>
  </span>

  <!-- Drawn in CSS rather than as an svg: an inline svg inside this button
       picks up a box in WebKit that no rule of ours asks for, and a ring is a
       gradient and a mask anyway. -->
  <span class="ring" data-tone={known ? tone : undefined} style:--pct={value}>
    {#if known}
      <span class="fill"></span>
    {/if}
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

  /* No focus rings anywhere in the island. The panel never takes the keyboard
     (tech.md 6.7), and WebKit draws the ring in the user's system accent
     colour, which reads as somebody else's element sitting on the sign. */
  .mark:focus,
  .mark:focus-visible,
  .mark svg {
    outline: none;
  }

  /* The click belongs to the button, never to a child. */
  .mark span {
    pointer-events: none;
  }

  /* One colour in every state. A sign is recognised by its colour, and
     swapping it to carry a status makes it somebody else's sign each time.
     Presence carries the status instead. tech.md 9. */
  .glyph {
    display: flex;
    color: var(--brand);
    opacity: 0.55;
    /* Opacity only. The shape underneath belongs to Shape and never moves for
       a hover. tech.md 6.10. */
    transition: opacity 160ms ease-out;
  }

  .mark[data-status='working'] .glyph {
    opacity: 1;
  }

  /* Waiting is the one state that costs the user time, so it is the one state
     that moves. Never filter or backdrop-filter: those repaint everything
     under the window on every frame. tech.md 6.10. */
  .mark[data-status='waiting'] .glyph {
    opacity: 1;
    animation: breathe 1600ms ease-in-out infinite;
  }

  .mark:hover .glyph {
    opacity: 1;
  }

  .ring {
    position: relative;
    box-sizing: border-box;
    width: 15px;
    height: 15px;
    border: 2px solid var(--hairline);
    border-radius: 50%;
    color: var(--accent);
  }

  /* The same four thresholds UsageBar draws, because two readouts of one
     number that disagree are worse than one of them missing. tech.md 9. */
  .ring[data-tone='warn'] {
    color: var(--warn);
  }
  .ring[data-tone='orange'] {
    color: var(--orange);
  }
  .ring[data-tone='danger'] {
    color: var(--danger);
  }

  /* The arc starts at twelve o'clock and the mask cuts the disc back to a
     ring of the same width as the track it sits on. */
  .fill {
    position: absolute;
    inset: -2px;
    border-radius: 50%;
    background: conic-gradient(currentColor calc(var(--pct) * 1%), transparent 0);
    -webkit-mask: radial-gradient(
      closest-side,
      transparent calc(100% - 2px),
      #000 calc(100% - 2px)
    );
    mask: radial-gradient(closest-side, transparent calc(100% - 2px), #000 calc(100% - 2px));
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
