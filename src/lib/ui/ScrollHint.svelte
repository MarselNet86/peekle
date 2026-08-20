<script lang="ts">
  let { visible = false, onclick }: { visible?: boolean; onclick?: () => void } = $props();
</script>

<!-- A control, not a picture. Pressing it was the first thing anyone tried,
     and for two versions it did nothing. tech.md 6.12. -->
<button
  class="hint"
  class:visible
  type="button"
  tabindex={visible ? 0 : -1}
  aria-label="Scroll to the newest message"
  onclick={() => onclick?.()}
>
  <svg viewBox="0 0 12 8" width="11" height="7" aria-hidden="true">
    <path d="M1 1l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1.5" />
  </svg>
</button>

<style>
  .hint {
    display: flex;
    justify-content: center;
    align-items: center;
    width: 100%;
    border: none;
    background: transparent;
    color: var(--text-dim);
    /* Room to breathe. Pinned to the edge of the shape it read as a glitch in
       the corner rather than as a control. */
    padding: 9px 0 7px;
    opacity: 0;
    transition: opacity 120ms ease;
    pointer-events: none;
    cursor: pointer;
  }

  .visible {
    opacity: 1;
    pointer-events: auto;
  }

  .hint:hover {
    color: var(--text);
  }

  /* No focus rings anywhere in the island: the panel never takes the
     keyboard, and WebKit paints the ring in the system accent. tech.md 9. */
  .hint:focus,
  .hint:focus-visible {
    outline: none;
  }
</style>
