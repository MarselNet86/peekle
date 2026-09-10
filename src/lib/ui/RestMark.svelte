<script lang="ts">
  import type { RestStatus } from '$lib/logic/rest';
  import { REST_SIDE } from '$lib/logic/shape';
  import { usageTone } from '$lib/logic/usage';
  import UsageDial from './UsageDial.svelte';

  let {
    status,
    pct,
    badge = null,
    onopen,
  }: {
    status: RestStatus;
    pct: number | null;
    /** The percent stepping out beside the ring, or null while none is.
     * tech.md 6.18. */
    badge?: number | null;
    onopen: () => void;
  } = $props();

  // Kept after the badge is taken away, so the number can leave on a
  // transition rather than blink out of the markup. tech.md 6.18.
  let last = $state<number | null>(null);
  const standing = $derived(badge !== null);
  const number = $derived(badge ?? last ?? 0);
  const tone = $derived(usageTone(number));

  $effect(() => {
    if (badge !== null) last = badge;
  });

  const known = $derived(pct !== null);
  const value = $derived(known ? Math.round(Math.min(100, Math.max(0, pct as number))) : 0);
  const labels: Record<RestStatus, string> = {
    idle: 'Peekle is running',
    working: 'Claude is working',
    waiting: 'Claude is waiting on you',
  };
  const usageLabel = $derived(known ? `, ${value}% of the 5h window used` : '');
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
  <!-- Two strokes at rest. While the agent works they give way to one stroke
       stepping through `|`, `\`, `-`, `/`: the console spinner, where a frame
       is replaced rather than turned. tech.md 6.12. -->
  <span class="glyph">
    <svg viewBox="0 0 14 12" width="14" height="12" aria-hidden="true">
      <g class="sign">
        <path
          class="stroke first"
          d="M4 10.6L6.9 1.4"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
        />
        <path
          class="stroke second"
          d="M9.1 10.6L12 1.4"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
        />
      </g>
      <!-- Four glyphs, not one turning stroke. A console spinner replaces the
           character: the vertical bar is tall, the dash is short and wide, and
           the eye reads a swap rather than a rotation. tech.md 6.12. -->
      <g class="spin">
        <path class="frame f1" d="M7 1.9L7 10.1" />
        <path class="frame f2" d="M4.3 10.1L9.7 1.9" />
        <path class="frame f3" d="M3.1 6L10.9 6" />
        <path class="frame f4" d="M4.3 1.9L9.7 10.1" />
      </g>
    </svg>
  </span>

  <!-- The number stands to the left of the ring, the way the system writes
       `10%` to the left of the battery. It is positioned rather than laid
       out: the ring rides the growing edge of the shape on the spring of
       6.10, and a number in the flow would shove it there instead. -->
  <span class="usage">
    <span class="pct" class:standing data-tone={tone} aria-hidden="true">{number}%</span>
    <UsageDial {pct} size={15} />
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

  .usage {
    position: relative;
    display: flex;
    align-items: center;
  }

  .pct {
    position: absolute;
    right: calc(100% + 6px);
    top: 50%;
    opacity: 0;
    /* Content follows the shape rather than arriving with it, and leaves
       before it. tech.md 6.10 and 6.18. */
    transform: translate(7px, -50%);
    transition:
      opacity 150ms ease-out,
      transform 240ms cubic-bezier(0.22, 1, 0.36, 1);
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.2px;
    white-space: nowrap;
    color: var(--accent);
  }

  .pct.standing {
    opacity: 1;
    transform: translate(0, -50%);
    transition-delay: 60ms;
  }

  /* The number carries the same thresholds as the ring beside it: two
     readings of one number must never disagree on colour. tech.md 9. */
  .pct[data-tone='warn'] {
    color: var(--warn);
  }
  .pct[data-tone='orange'] {
    color: var(--orange);
  }
  .pct[data-tone='danger'] {
    color: var(--danger);
  }

  @media (prefers-reduced-motion: reduce) {
    .pct {
      transition: none;
    }
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

  /* One glyph on screen at a time. Each frame owns a quarter of the cycle and
     holds, so nothing fades and nothing turns: the character is replaced, the
     way a console spinner does it. tech.md 6.12. */
  .spin {
    display: none;
  }

  .frame {
    opacity: 0;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
  }

  .mark[data-status='working'] .sign {
    display: none;
  }

  .mark[data-status='working'] .spin {
    display: block;
  }

  .mark[data-status='working'] .frame {
    /* steps(1, end) holds each keyframe interval at its own value, so opacity
       jumps rather than ramps. */
    animation: flick 640ms steps(1, end) infinite;
  }

  /* Qualified to match the rule above: the `animation` shorthand resets the
     delay, so a plainer selector here would lose and stack all four frames in
     one phase. */
  .mark[data-status='working'] .f2 {
    animation-delay: 160ms;
  }
  .mark[data-status='working'] .f3 {
    animation-delay: 320ms;
  }
  .mark[data-status='working'] .f4 {
    animation-delay: 480ms;
  }

  @keyframes flick {
    0% {
      opacity: 1;
    }
    25% {
      opacity: 0;
    }
    100% {
      opacity: 0;
    }
  }

  /* Waiting is the one state that costs the user time, so it is the one state
     that moves, and the one that changes colour. The sign is green for Peekle
     and for everything the agent does on its own; a question standing
     unanswered is neither, and the eye has to find it from across a screen.
     Never filter or backdrop-filter: those repaint everything under the
     window on every frame. tech.md 6.7, 6.10 and 6.14. */
  .mark[data-status='waiting'] .glyph {
    opacity: 1;
    color: var(--waiting);
    animation: breathe 1600ms ease-in-out infinite;
  }

  /* One stroke goes up and comes back down, then the other: a walk, not a
     jitter. The two halves of the cycle are the two strokes, so the sign is
     never still and never leaves its own footprint. */
  .mark[data-status='waiting'] .stroke {
    transform-box: fill-box;
    transform-origin: center;
    animation: hop 1400ms ease-in-out infinite;
  }

  .mark[data-status='waiting'] .second {
    animation-delay: 700ms;
  }

  @keyframes hop {
    0%,
    30%,
    100% {
      transform: translateY(0);
    }
    15% {
      transform: translateY(-2.2px);
    }
  }

  .mark:hover .glyph {
    opacity: 1;
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

    /* The colour still says it, and the colour does not move. */
    .mark[data-status='waiting'] .stroke {
      animation: none;
    }

    /* Without motion the spinner would be four glyphs stacked, so one holds. */
    .mark[data-status='working'] .frame {
      animation: none;
    }
    .mark[data-status='working'] .f1 {
      opacity: 1;
    }
  }
</style>
