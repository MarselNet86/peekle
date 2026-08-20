<script lang="ts">
  /** How long a letter takes to appear, and how long a finished word holds. */
  const LETTER_MS = 55;
  const HOLD_MS = 900;

  let { words = ['Working', 'Reading', 'Thinking', 'Writing', 'Checking'] }: { words?: string[] } =
    $props();

  let shown = $state('');

  // A dialogue that sits still while the agent works reads as broken, so the
  // line types rather than blinks. tech.md 6.12.
  $effect(() => {
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
</script>

<div class="typing" aria-live="polite">
  <span class="star" aria-hidden="true">✳</span>
  <span class="word">{shown}</span><span class="caret" aria-hidden="true"></span>
</div>

<style>
  .typing {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    font-size: 13px;
    color: var(--text-dim);
  }

  .star {
    color: var(--brand);
    font-size: 12px;
    line-height: 1;
    /* The one thing that moves on its own while a word is being typed. */
    animation: pulse 1600ms ease-in-out infinite;
  }

  .word {
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
    .star,
    .caret {
      animation: none;
    }
  }
</style>
