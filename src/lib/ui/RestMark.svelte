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
  <!-- Two strokes at rest. While the agent works they give way to one stroke
       stepping through `|`, `\`, `-`, `/`: the console spinner, where a frame
       is replaced rather than turned. tech.md 6.12. -->
  <span class="glyph">
    <svg viewBox="0 0 14 12" width="14" height="12" aria-hidden="true">
      <g class="sign">
        <path d="M4 10.6L6.9 1.4" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
        <path
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

  <!-- Never call this `dial` a `ring`: Tailwind owns that class name and paints
       its own box-shadow over anything wearing it. Svelte scoping does not save
       you, the element still carries the bare class. -->
  <span class="dial" data-tone={known ? tone : undefined} style:--pct={value}>
    <span class="track"></span>
    {#if known}
      <span class="fill"></span>
    {/if}
    <span class="hole"></span>
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
     that moves. Never filter or backdrop-filter: those repaint everything
     under the window on every frame. tech.md 6.10. */
  .mark[data-status='waiting'] .glyph {
    opacity: 1;
    animation: breathe 1600ms ease-in-out infinite;
  }

  .mark:hover .glyph {
    opacity: 1;
  }

  .dial {
    position: relative;
    width: 15px;
    height: 15px;
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

  /* The empty dial. Dimmed rather than hairline: that token is meant for one
     pixel separators and disappears at two pixels on pure black. */
  .track {
    inset: 0;
    box-sizing: border-box;
    border: 2px solid var(--text-dim);
    opacity: 0.3;
  }

  /* The arc starts at twelve o'clock and runs clockwise. It is a full disc cut
     back by the hole rather than a mask: masks are the one part of this that
     the app webview renders differently from every browser we test in. */
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

    /* Without motion the spinner would be four glyphs stacked, so one holds. */
    .mark[data-status='working'] .frame {
      animation: none;
    }
    .mark[data-status='working'] .f1 {
      opacity: 1;
    }
  }
</style>
