<script lang="ts">
  let {
    name,
    title,
    pressed = false,
    onclick,
  }: {
    /** Which sign it wears. Drawn here, because a feature that draws its own
     * svg is a feature that drew its own button. tech.md 9. */
    name: 'gear';
    /** What it does, for the pointer and for a reader who cannot see a gear. */
    title: string;
    /** Held lit while what it opened is open. A control that opens something
     * and then looks untouched reads as one that did nothing. */
    pressed?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<!-- A sign and no word. It stands where a word would not fit and says what it
     does through `title` and `aria-label`. tech.md 9. -->
<button
  type="button"
  {title}
  aria-label={title}
  aria-pressed={pressed}
  class:pressed
  onclick={() => onclick?.()}
>
  {#if name === 'gear'}
    <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
      <circle cx="8" cy="8" r="2.4" fill="none" stroke="currentColor" stroke-width="1.4" />
      <path
        d="M8 1.4v1.7M8 12.9v1.7M14.6 8h-1.7M3.1 8H1.4M12.7 3.3l-1.2 1.2M4.5 11.5l-1.2 1.2M12.7 12.7l-1.2-1.2M4.5 4.5L3.3 3.3"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
      />
    </svg>
  {/if}
</button>

<style>
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 28px;
    height: 28px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    transition:
      color 120ms ease,
      border-color 120ms ease,
      background 120ms ease;
  }

  button:hover {
    color: var(--text);
    background: var(--bubble);
  }

  /* Lit while what it opened stands open, so the row says where the reader
     is rather than only how they got there. */
  .pressed {
    color: var(--text);
    border-color: var(--hairline);
    background: var(--bubble);
  }
</style>
