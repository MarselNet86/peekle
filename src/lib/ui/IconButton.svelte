<script lang="ts">
  import { ArrowLeft, Settings } from '@lucide/svelte';

  let {
    name,
    title,
    pressed = false,
    onclick,
  }: {
    /** Which sign it wears. The signs come from the icon set, not from hand
     * drawn paths: a gear drawn by hand comes out a sun, which is what the
     * first cut of this did. tech.md 9. */
    name: 'settings' | 'back';
    /** What it does, for the pointer and for a reader who sees no icon. */
    title: string;
    /** Held lit while what it opened is open. A control that opens something
     * and then looks untouched reads as one that did nothing. */
    pressed?: boolean;
    onclick?: () => void;
  } = $props();

  const Sign = $derived(name === 'settings' ? Settings : ArrowLeft);
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
  <Sign size={15} strokeWidth={1.75} aria-hidden="true" />
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
