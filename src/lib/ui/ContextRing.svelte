<script lang="ts">
  /**
   * The context ring, where the original keeps it: the far left of the row
   * under the field. It shows how full the window is, says so on hover, and
   * pressing it compacts. tech.md 6.15.
   */
  import UsageDial from './UsageDial.svelte';

  let {
    pct,
    title,
    live = false,
    pending = false,
    onclick,
  }: {
    /** Null until the session has answered once: nothing has been asked of
     * the window yet, which is not the same claim as zero. */
    pct: number | null;
    title: string;
    live?: boolean;
    /** A compact was asked for. It is confirmed by the number falling, which
     * takes a while, so the ring waits visibly. */
    pending?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<!-- Never disabled while there is a number to show. A ring that cannot be
     pressed answers a press with nothing at all, and "nothing happened" is
     the one answer a control must not give: on a chat we do not run, the
     press is answered by the block above the feed. tech.md 6.15 and 9. -->
<button
  class="ring"
  class:pending
  class:reading={!live}
  disabled={pct === null}
  {title}
  aria-label={title}
  onclick={() => onclick?.()}
>
  <UsageDial {pct} size={15} track={false} />
</button>

<style>
  .ring {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 999px;
    background: transparent;
    padding: 0;
    cursor: pointer;
    transition: background 140ms ease-out;
  }

  .ring:hover:not(:disabled) {
    background: var(--bubble);
  }

  .ring:disabled {
    cursor: default;
  }

  /* Dimmed the way every other value in the row is dimmed when it only
     reads, so the row says the same thing in one voice. tech.md 6.15. */
  .ring.reading {
    opacity: 0.55;
  }

  /* A compact takes minutes and confirms itself by the number falling, so the
     ring waits visibly rather than sitting as if nothing was asked. 6.15. */
  .ring.pending {
    opacity: 0.5;
  }

  .ring:focus,
  .ring:focus-visible {
    outline: none;
  }
</style>
