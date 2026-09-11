<script lang="ts">
  import { ArrowLeft, Bug, Settings, X } from '@lucide/svelte';

  let {
    name,
    title,
    hint,
    pressed = false,
    onclick,
  }: {
    /** Which sign it wears. The signs come from the icon set, not from hand
     * drawn paths: a gear drawn by hand comes out a sun, which is what the
     * first cut of this did. tech.md 9. */
    name: 'settings' | 'back' | 'close' | 'bug';
    /** What it does, for the pointer and for a reader who sees no icon. */
    title: string;
    /** One line, raised under the button while the pointer is on it or the
     * keyboard has it. Drawn here rather than left to the system: a macOS
     * tooltip comes up on the key window, and the panel does not become key
     * unless it needs to, and it is a second window beside the black shape.
     * tech.md 6.22. */
    hint?: string;
    /** Held lit while what it opened is open. A control that opens something
     * and then looks untouched reads as one that did nothing. */
    pressed?: boolean;
    onclick?: () => void;
  } = $props();

  const SIGNS = { settings: Settings, back: ArrowLeft, close: X, bug: Bug };
  const Sign = $derived(SIGNS[name]);

  const id = $props.id();
  let over = $state(false);
  const telling = $derived(over && !!hint);
</script>

<!-- A sign and no word. It stands where a word would not fit and says what it
     does through `title` and `aria-label`. tech.md 9. -->
<span class="holder">
  <button
    type="button"
    {title}
    aria-label={title}
    aria-pressed={pressed}
    aria-describedby={telling ? id : undefined}
    class:pressed
    onclick={() => onclick?.()}
    onpointerenter={() => (over = true)}
    onpointerleave={() => (over = false)}
    onfocus={() => (over = true)}
    onblur={() => (over = false)}
  >
    <Sign size={15} strokeWidth={1.75} aria-hidden="true" />
  </button>
  {#if telling}
    <!-- A layer, never a row: a hint that pushes the list down on hover moves
         the thing the reader was about to press. -->
    <span class="hint" {id} role="tooltip">{hint}</span>
  {/if}
</span>

<style>
  .holder {
    position: relative;
    display: inline-flex;
    flex: none;
  }

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

  /* Under the button and pinned to its right edge, because the buttons that
     carry a hint stand in the right corner: a line centred on a corner button
     hangs off the side of the shape. */
  .hint {
    position: absolute;
    top: calc(100% + 2px);
    right: 0;
    z-index: 3;
    /* Sized by its own words, not by the button it hangs from. The box an
       absolutely positioned child is measured against is the button -- 28
       pixels -- so shrink-to-fit wraps it into a column one letter wide, and
       `nowrap` instead made the words run off the ground they were drawn on.
       `max-content` asks for the width of the line; the ceiling then wraps it
       rather than spilling it. */
    width: max-content;
    max-width: 300px;
    padding: 5px 8px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    /* Its own backing, and an opaque one: the line stands over the list, and
       two texts through each other is neither. The same ground `NoteBlock`
       uses, for the same reason. */
    background: rgba(28, 28, 32, 0.94);
    backdrop-filter: blur(20px);
    color: var(--text);
    font-size: 11px;
    line-height: 1.3;
    white-space: normal;
    overflow-wrap: anywhere;
    pointer-events: none;
  }
</style>
