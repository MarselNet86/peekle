<script lang="ts">
  let {
    label,
    variant = 'ghost',
    disabled = false,
    wide = false,
    busy = false,
    onclick,
  }: {
    label: string;
    variant?: 'primary' | 'ghost' | 'connect';
    disabled?: boolean;
    /** Fills the row it sits in. For a control that is the only thing there. */
    wide?: boolean;
    /** Working on the last press. A label that merely changes its word reads
     * as a button that did nothing, so the wait gets a moving part of its
     * own, and the button stops taking presses while it turns. tech.md 9. */
    busy?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<!-- The only button in the product. tech.md 9.
     No autofocus and no tabindex games: a button that takes focus would pull
     the keyboard out of whatever the user is actually using. tech.md 6.7. -->
<button
  type="button"
  data-variant={variant}
  class:wide
  class:busy
  disabled={disabled || busy}
  aria-busy={busy}
  onclick={() => onclick?.()}
>
  {#if busy}
    <span class="spinner" aria-hidden="true"></span>
  {/if}
  {label}
</button>

<style>
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    flex: none;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 12px;
    line-height: 1;
    padding: 7px 12px;
    cursor: pointer;
    transition:
      color 120ms ease,
      border-color 120ms ease,
      background 120ms ease;
  }

  button:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--text-dim);
  }

  button[data-variant='primary'] {
    color: var(--notch);
    background: var(--accent);
    border-color: var(--accent);
  }

  button[data-variant='primary']:hover:not(:disabled) {
    color: var(--notch);
    filter: brightness(1.08);
  }

  /* Connecting is the one action that gets the product's own green, the same
     one the sign in the resting mark wears. tech.md 9. */
  button[data-variant='connect'] {
    color: var(--notch);
    background: var(--brand);
    border-color: var(--brand);
    font-weight: 600;
  }

  button[data-variant='connect']:hover:not(:disabled) {
    color: var(--notch);
    filter: brightness(1.08);
  }

  button.wide {
    flex: 1;
    width: 100%;
    padding: 11px 12px;
    font-size: 13px;
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  /* A filled button at 40% over black is a muddy green that reads as a
     rendering fault. Unavailable, so the accent goes quiet rather than dim --
     and it stays the accent, because a primary action that becomes
     indistinguishable from Cancel loses the reader the thing they came for.
     tech.md 9. */
  button[data-variant='connect']:disabled,
  button[data-variant='primary']:disabled {
    opacity: 1;
    color: var(--brand-dim);
    background: var(--brand-soft);
    border-color: transparent;
  }

  /* A ring with a gap, turning. The one moving part the product has outside
     the feed, and it exists because a wait with no moving part is read as a
     press that was lost. tech.md 9. */
  .spinner {
    width: 11px;
    height: 11px;
    flex: none;
    border-radius: 50%;
    border: 1.5px solid currentColor;
    border-top-color: transparent;
    opacity: 0.85;
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Motion is a preference, and a spinner that cannot turn still has to say it
     is working, so it holds the gap still instead. */
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
