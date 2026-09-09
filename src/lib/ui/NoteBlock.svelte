<script lang="ts">
  /**
   * An answer to a press that cannot do what was asked: what is so, and what
   * to do about it. tech.md 9 and 6.15.
   *
   * Built as an answer rather than as a failure. A single dim line at the
   * width of the island reads as an error string, and the reader takes it as
   * something broken instead of something explained. So the fact stands in
   * the reading colour on the bubble the feed already uses, and the way out
   * sits under it, quieter: it is only worth reading if the fact is
   * unwelcome.
   *
   * It stands above the feed rather than under it. The press it answers
   * happens in the row by the input, and an answer that appears below the
   * fold of what the eye is doing is an answer nobody reads. A hairline
   * underneath leaks its own term away, the same way the permission panel
   * shows its twenty seconds (6.7): a clock that is visible does not have to
   * be guessed at. Hovering holds both the hairline and the clock, because a
   * person who is reading it is not spending it.
   */
  import { NOTE_EXIT_MS, NOTE_TTL } from '$lib/logic/agent';
  import IconButton from './IconButton.svelte';

  let {
    fact,
    how = null,
    ttl = NOTE_TTL,
    onclose,
  }: {
    fact: string;
    how?: string | null;
    /** How long it stands, in milliseconds. `0` stands until it is closed. */
    ttl?: number;
    onclose?: () => void;
  } = $props();

  /** Playing the way out, so the block leaves the way it arrived. */
  let leaving = $state(false);

  // What is left of the term, and when this run of it started. Kept rather
  // than recomputed from a start time, because a hover pauses the clock and
  // the pause has to survive into the next run. Filled on the first run: the
  // term belongs to the block as it was mounted, and a caller that swaps it
  // mid-read would be moving the goalposts on someone who is reading.
  let left: number | null = null;
  let ran = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function stop() {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
    left = Math.max(0, (left ?? 0) - (Date.now() - ran));
  }

  function run() {
    if (ttl <= 0 || leaving || timer !== null) return;
    left ??= ttl;
    ran = Date.now();
    timer = setTimeout(leave, left);
  }

  function leave() {
    stop();
    leaving = true;
    // Long enough to play the way out, and nothing waits on it: the block is
    // gone from the layout the moment the caller drops it.
    setTimeout(() => onclose?.(), NOTE_EXIT_MS);
  }

  $effect(() => {
    run();
    return stop;
  });
</script>

<div
  class="note"
  class:leaving
  role="status"
  onmouseenter={stop}
  onmouseleave={run}
  style:--secs="{ttl / 1000}s"
>
  <div class="said">
    <span class="fact">{fact}</span>
    {#if how}<span class="how">{how}</span>{/if}
  </div>
  <IconButton name="close" title="Dismiss" onclick={leave} />
  {#if ttl > 0}
    <span class="leak" aria-hidden="true"></span>
  {/if}
</div>

<style>
  .note {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0 0 8px;
    padding: 9px 7px 9px 11px;
    overflow: hidden;
    border-radius: 10px;
    background: var(--bubble);
    font-size: 12px;
    line-height: 1.4;
    /* The panel comes down from the edge it belongs to and settles without a
       bounce: the curve is the one Apple uses for a sheet arriving. */
    animation: arrive 260ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .note.leaving {
    animation: leave 180ms cubic-bezier(0.32, 0.72, 0, 1) forwards;
  }

  .said {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .fact {
    color: var(--text);
  }

  .how {
    color: var(--text-dim);
  }

  .leak {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    /* Neutral, not green: green means Peekle in this product, and a clock is
       not the product saying anything. tech.md 9. */
    background: rgba(255, 255, 255, 0.3);
    transform-origin: left center;
    animation: leak var(--secs) linear forwards;
  }

  /* Reading it does not spend it. */
  .note:hover .leak {
    animation-play-state: paused;
  }

  @keyframes arrive {
    from {
      transform: translateY(-10px);
      opacity: 0;
    }
  }

  @keyframes leave {
    to {
      transform: translateY(-10px);
      opacity: 0;
    }
  }

  @keyframes leak {
    from {
      transform: scaleX(1);
    }
    to {
      transform: scaleX(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .note {
      animation: fade 160ms linear;
    }

    .note.leaving {
      animation: fade 160ms linear reverse forwards;
    }

    .leak {
      animation: none;
      transform: scaleX(0.02);
    }
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
