<script lang="ts">
  import { untrack } from 'svelte';

  import { ASK_PIXEL, ASK_PIXELS } from '$lib/logic/ask-sign';
  import type { RestStatus } from '$lib/logic/rest';
  import { SIGN_BOX, SIGN_STROKES, SIGN_WEIGHT } from '$lib/logic/sign';
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
    compacting: 'Claude is compacting the chat',
  };
  const usageLabel = $derived(known ? `, ${value}% of the 5h window used` : '');

  /** The one state that is about the person rather than about the agent, and
   * the one that swaps the glyph rather than only its colour. tech.md 6.7. */
  const asking = $derived(status === 'waiting');

  /** How long the strokes hop for when the state changes under them. Long
   * enough to be seen from across a screen, short enough not to be a state of
   * its own. tech.md 6.21. */
  const HOP_MS = 620;

  // The colour says what is happening; the hop says it just changed. Without
  // it a compact that ends while nobody is looking at the notch is a green
  // sign that was orange a moment ago, and nothing was ever seen to happen.
  let turned = $state(false);
  let seen = $state<RestStatus | null>(null);

  $effect(() => {
    const next = status;
    const before = untrack(() => seen);
    if (before === next) return;
    seen = next;
    // The first paint is not a change. An island that hops on every launch
    // is an island that hops for nothing.
    if (before === null) return;

    turned = true;
    const timer = setTimeout(() => (turned = false), HOP_MS);
    return () => clearTimeout(timer);
  });
</script>

<!-- The whole resting shape is the target. Its middle is behind the camera
     housing, so asking the user to hit either end would be unfair. tech.md 6.7. -->
<button
  class="mark"
  class:turned
  data-status={status}
  style:--side="{REST_SIDE}px"
  aria-label="{labels[status]}{usageLabel}. Open the session list"
  onclick={onopen}
>
  <!-- Two strokes at rest. While the agent works they give way to one stroke
       stepping through `|`, `\`, `-`, `/`: the console spinner, where a frame
       is replaced rather than turned. tech.md 6.12. A standing question takes
       the whole glyph away and puts a question mark in its place: the sign
       says Peekle, and what the island has to say here is not Peekle.
       tech.md 6.7. -->
  <span class="glyph">
    <svg
      viewBox="0 0 {SIGN_BOX.width} {SIGN_BOX.height}"
      width={SIGN_BOX.width}
      height={SIGN_BOX.height}
      aria-hidden="true"
    >
      {#if asking}
        <!-- Whole pixels, drawn from `logic/ask-sign.ts`: at eight by twelve a
             drawn curve turns to mush, and a bitmap is what a terminal would
             have written anyway. It is swapped in rather than faded in, the
             way the spinner replaces a frame, and it never moves -- the breath
             is on the colour, and a pixel shifted by a fraction is a blurred
             pixel. tech.md 6.7. -->
        <g class="ask" shape-rendering="crispEdges">
          {#each ASK_PIXELS as pixel (`${pixel.x},${pixel.y}`)}
            <rect
              x={pixel.x}
              y={pixel.y}
              width={ASK_PIXEL}
              height={ASK_PIXEL}
              fill="currentColor"
            />
          {/each}
        </g>
      {:else}
        <g class="sign">
          <!-- The geometry is the one in `logic/sign.ts`, not a copy of it: the
               empty dialogue draws the same two strokes, and a sign drawn twice
               by hand is a sign that drifts. tech.md 9. -->
          {#each SIGN_STROKES as stroke, index (stroke)}
            <path
              class="stroke"
              class:first={index === 0}
              class:second={index === 1}
              d={stroke}
              stroke="currentColor"
              stroke-width={SIGN_WEIGHT}
              stroke-linecap="round"
            />
          {/each}
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
      {/if}
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
     that changes its colour and the only one that changes its glyph. The sign
     is green for Peekle and for everything the agent does on its own; a
     question standing unanswered is neither, and the eye has to find it from
     across a screen. The breath is all of the movement, and opacity is all of
     the breath: the wave that ran through the two strokes until v80.3 went
     with them, and a pixel glyph that scales or slides is a pixel glyph with
     soft edges. Never filter or backdrop-filter: those repaint everything
     under the window on every frame. tech.md 6.7, 6.10 and 6.14. */
  .mark[data-status='waiting'] .glyph {
    opacity: 1;
    color: var(--waiting);
    animation: breathe 1600ms ease-in-out infinite;
  }

  @keyframes wave {
    0%,
    100% {
      transform: translateY(1.1px);
    }
    50% {
      transform: translateY(-1.1px);
    }
  }

  /* A compact is not the agent working and not the agent waiting: it is the
     CLI folding the conversation up, for minutes at a time, while nothing
     else moves. Orange rather than yellow or red, which mean warning and
     fault, and neither is true here. tech.md 6.21. */
  .mark[data-status='compacting'] .glyph {
    opacity: 1;
    color: var(--orange);
  }

  /* The wave the waiting mark wore until v80.3, a shade quicker: work moves,
     waiting breathes, and a compact rides this. No breath under it, so it and
     waiting never read alike. */
  .mark[data-status='compacting'] .stroke {
    transform-box: fill-box;
    transform-origin: center;
    animation: wave 1200ms ease-in-out infinite;
  }

  .mark[data-status='compacting'] .second {
    animation-delay: 600ms;
  }

  /* One hop on every change of state, the second stroke behind the first, so
     the colour is seen changing rather than found already changed. Last in
     the file on purpose: it has the same specificity as the state rules above
     and has to win over them for as long as it stands. tech.md 6.21. */
  .mark.turned .stroke {
    transform-box: fill-box;
    transform-origin: center;
    animation: hop 620ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .mark.turned .second {
    animation-delay: 140ms;
  }

  @keyframes hop {
    0%,
    100% {
      transform: translateY(0);
    }
    40% {
      transform: translateY(-3px);
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
    .mark[data-status='compacting'] .stroke,
    .mark.turned .stroke {
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
