<script lang="ts">
  import { LANGUAGE_CHOICES, LANGUAGE_TITLE } from '$lib/logic/language';
  import type { Language } from '$lib/types/generated/Language';

  let {
    value = null,
    onpick,
  }: {
    /** The card already pressed, held while the screen gives way. */
    value?: Language | null;
    onpick?: (language: Language) => void;
  } = $props();
</script>

<!-- The first thing a fresh install asks, and the only thing on the screen
     while it asks: a title in both languages and two cards. tech.md 6.28. -->
<div class="language">
  <div class="stage">
    <h2>
      <span lang="ru">{LANGUAGE_TITLE.ru}</span>
      <span class="second" lang="en">{LANGUAGE_TITLE.en}</span>
    </h2>

    <div class="cards" role="radiogroup" aria-label="{LANGUAGE_TITLE.ru} · {LANGUAGE_TITLE.en}">
      {#each LANGUAGE_CHOICES as choice, index (choice.id)}
        <button
          type="button"
          class="card"
          role="radio"
          lang={choice.id}
          aria-checked={value === choice.id}
          class:chosen={value === choice.id}
          class:faded={value !== null && value !== choice.id}
          disabled={value !== null}
          style="--order: {index}"
          onclick={() => onpick?.(choice.id)}
        >
          <span class="flag" aria-hidden="true">{choice.flag}</span>
          <span class="name">{choice.name}</span>
          <span class="code">{choice.id}</span>
          {#if value === choice.id}
            <span class="tick" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="12" height="12">
                <path
                  d="M5 12.5l4.2 4.2L19 7"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="3"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
          {/if}
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .language {
    display: flex;
    justify-content: center;
    width: 100%;
  }

  /* The same entrance every screen in the island rides. Opacity and offset
     only. tech.md 6.10. */
  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    animation: arrive 280ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  h2 {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text);
  }

  .second {
    font-size: 13px;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--text-dim);
  }

  .cards {
    display: flex;
    gap: 12px;
    margin-top: 22px;
  }

  /* No outline: a card is lit by its fill, and a picked one by a green edge,
     the colour the product says yes in. tech.md 9. */
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 148px;
    padding: 20px 12px 16px;
    border: 1px solid transparent;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    /* One after the other, on the curve of the stage. */
    animation: rise 360ms cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: calc(90ms + var(--order) * 70ms);
    transition:
      transform 180ms cubic-bezier(0.22, 1, 0.36, 1),
      background 160ms ease,
      border-color 160ms ease,
      opacity 200ms ease;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .card:hover:not(:disabled) {
    transform: translateY(-3px);
    background: rgba(255, 255, 255, 0.12);
  }

  .card:active:not(:disabled) {
    transform: scale(0.97);
  }

  .card:disabled {
    cursor: default;
  }

  .card:focus,
  .card:focus-visible {
    outline: none;
  }

  .card.chosen {
    border-color: rgba(48, 209, 88, 0.55);
    background: var(--brand-soft);
  }

  .card.faded {
    opacity: 0.4;
  }

  .flag {
    font-size: 40px;
    line-height: 1;
  }

  .chosen .flag {
    animation: pop 520ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes pop {
    0% {
      transform: scale(0.86);
    }
    60% {
      transform: scale(1.12);
    }
    100% {
      transform: scale(1);
    }
  }

  .name {
    margin-top: 4px;
    font-size: 14px;
    font-weight: 600;
  }

  .code {
    font-size: 11px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .tick {
    position: absolute;
    top: 9px;
    right: 9px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--brand);
    color: var(--notch);
    animation: pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @media (prefers-reduced-motion: reduce) {
    .stage,
    .card,
    .chosen .flag,
    .tick {
      animation: none;
    }

    .card {
      transition: none;
    }
  }
</style>
