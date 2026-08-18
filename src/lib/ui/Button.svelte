<script lang="ts">
  let {
    label,
    variant = 'ghost',
    disabled = false,
    onclick,
  }: {
    label: string;
    variant?: 'primary' | 'ghost';
    disabled?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<!-- The only button in the product. tech.md 9.
     No autofocus and no tabindex games: a button that takes focus would pull
     the keyboard out of whatever the user is actually using. tech.md 6.7. -->
<button type="button" data-variant={variant} {disabled} onclick={() => onclick?.()}>
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

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
