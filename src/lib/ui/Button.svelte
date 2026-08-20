<script lang="ts">
  let {
    label,
    variant = 'ghost',
    disabled = false,
    wide = false,
    onclick,
  }: {
    label: string;
    variant?: 'primary' | 'ghost' | 'connect';
    disabled?: boolean;
    /** Fills the row it sits in. For a control that is the only thing there. */
    wide?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<!-- The only button in the product. tech.md 9.
     No autofocus and no tabindex games: a button that takes focus would pull
     the keyboard out of whatever the user is actually using. tech.md 6.7. -->
<button type="button" data-variant={variant} class:wide {disabled} onclick={() => onclick?.()}>
  {label}
</button>

<style>
  button {
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
</style>
