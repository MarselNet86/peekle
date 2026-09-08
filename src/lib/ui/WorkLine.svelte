<script lang="ts">
  import { elapsedLabel } from '$lib/logic/work';

  /** How long a letter takes to appear, how long a finished word holds, and
   * how often the clock moves. tech.md 6.12. */
  const LETTER_MS = 55;
  const HOLD_MS = 900;
  const TICK_MS = 1000;

  let {
    running,
    from = null,
    to = null,
    words = ['Working', 'Reading', 'Thinking', 'Writing', 'Checking'],
  }: {
    running: boolean;
    from?: number | null;
    to?: number | null;
    words?: string[];
  } = $props();

  let shown = $state('');
  let now = $state(Date.now());

  // A dialogue that sits still while the agent works reads as broken, so the
  // line types rather than blinks. tech.md 6.12.
  $effect(() => {
    if (!running) {
      shown = '';
      return;
    }

    let word = 0;
    let letters = 0;
    let timer: ReturnType<typeof setTimeout>;

    const step = () => {
      const current = words[word % words.length] ?? '';
      letters += 1;
      shown = current.slice(0, letters);

      if (letters >= current.length) {
        letters = 0;
        word += 1;
        timer = setTimeout(step, HOLD_MS);
        return;
      }
      timer = setTimeout(step, LETTER_MS);
    };

    step();
    return () => clearTimeout(timer);
  });

  // The clock only runs while the work does. A finished line stands on the
  // two records that bound it and never asks the browser for the time.
  $effect(() => {
    if (!running) return;

    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), TICK_MS);
    return () => clearInterval(timer);
  });

  const span = $derived(
    from === null ? null : Math.max(0, (running ? now : (to ?? from)) - (from ?? 0)),
  );
  const clock = $derived(span === null ? null : elapsedLabel(span));
</script>

<!-- One line for a whole run of calls: the mark, the clock, the word. The
     calls themselves are the how, and the how is not the dialogue. tech.md 6.12. -->
<div class="work" class:running aria-live="polite">
  <span class="star" aria-hidden="true">✳</span>
  {#if running}
    {#if clock}
      <span class="clock">{clock}</span>
      <span class="dot" aria-hidden="true">·</span>
    {/if}
    <span class="typed"
      ><span class="word">{shown}</span><span class="caret" aria-hidden="true"></span></span
    >
  {:else}
    <span class="word">{clock ? `Worked for ${clock}` : 'Worked'}</span>
  {/if}
</div>

<style>
  .work {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 4px;
    font-size: 13px;
    color: var(--text-dim);
  }

  .star {
    color: var(--brand);
    font-size: 12px;
    line-height: 1;
  }

  /* The one thing that moves on its own while the agent is out. A line that
     stands still says nothing about whether anything is happening. */
  .running .star {
    animation: pulse 1600ms ease-in-out infinite;
  }

  /* The work is over, so the line stops asking for attention. It stays only
     to say how long it took. */
  .work:not(.running) .star {
    opacity: 0.45;
  }

  /* The caret belongs to the word, so the row's gap must not fall between
     them. */
  .typed {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .clock {
    /* Digits that change every second must not move the word beside them. */
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }

  .dot {
    color: var(--text-dim);
  }

  .running .word {
    color: var(--text);
  }

  .caret {
    width: 7px;
    height: 14px;
    background: var(--text-dim);
    animation: blink 1s steps(2, end) infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  @keyframes blink {
    0% {
      opacity: 1;
    }
    50% {
      opacity: 0;
    }
    100% {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .running .star,
    .caret {
      animation: none;
    }
  }
</style>
