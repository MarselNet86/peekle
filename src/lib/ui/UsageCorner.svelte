<script lang="ts">
  /**
   * The top right corner of an open dialogue: how much of the five hour
   * window, of the seven day window and of the context is gone.
   *
   * Three rings answering one question stand together. Two of them in the
   * header and the third down by the field made the eye collect the answer
   * from two ends of the same form. The context ring is still the button that
   * compacts, and still the only one. tech.md 6.12 and 6.15.
   */
  import UsageDial from './UsageDial.svelte';

  let {
    hour,
    week,
    context,
    contextTitle,
    live = false,
    pending = false,
    oncompact,
  }: {
    hour: number | null;
    week: number | null;
    /** Null until the session has answered once: nothing has been asked of
     * the window yet, which is not the same claim as zero. tech.md 6.15. */
    context: number | null;
    contextTitle: string;
    /** Whether the ring can compact. A chat another app runs cannot. */
    live?: boolean;
    /** A compact was asked for and is confirmed only by the number falling. */
    pending?: boolean;
    oncompact?: () => void;
  } = $props();
</script>

<div class="corner">
  <UsageDial pct={hour} label="5h" size={13} />
  <UsageDial pct={week} label="7d" size={13} />
  <button
    class="context"
    class:pending
    disabled={!live || context === null}
    title={contextTitle}
    aria-label={contextTitle}
    onclick={() => oncompact?.()}
  >
    <UsageDial pct={context} label="ctx" size={13} />
  </button>
</div>

<style>
  .corner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 4px;
  }

  .context {
    display: inline-flex;
    align-items: center;
    border: none;
    background: transparent;
    padding: 2px 4px;
    margin: -2px -4px;
    border-radius: 999px;
    cursor: pointer;
    transition: background 120ms ease;
  }

  .context:hover:not(:disabled) {
    background: var(--bubble);
  }

  /* A ring that cannot compact is a reading, so it promises nothing. */
  .context:disabled {
    cursor: default;
  }

  /* A compact takes minutes and confirms itself by the number falling, so the
     ring waits visibly rather than sitting as if nothing was asked. 6.15. */
  .context.pending {
    opacity: 0.5;
  }
</style>
