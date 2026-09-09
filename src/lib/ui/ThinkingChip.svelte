<script lang="ts">
  /**
   * Whether the session thinks before it answers, in a block beside the
   * model. One switch, because that is all the CLI has: `MAX_THINKING_TOKENS`
   * is read once at startup, so this is set on a session before it runs and
   * read afterwards. tech.md 6.20.
   */
  import Toggle from './Toggle.svelte';

  let {
    on = true,
    live = false,
    note = '',
    onchange,
    onnote,
  }: {
    on?: boolean;
    /** Whether it can still be set: a session Peekle has not started yet. */
    live?: boolean;
    note?: string;
    onchange?: (next: boolean) => void;
    onnote?: () => void;
  } = $props();

  let open = $state(false);
  let host = $state<HTMLElement | null>(null);

  function toggle() {
    if (!live) {
      onnote?.();
      return;
    }
    open = !open;
  }

  function away(event: MouseEvent) {
    if (!open || !host) return;
    if (!host.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onclick={away} />

<span class="thinking" bind:this={host}>
  <button
    class="chip"
    class:off={!on}
    class:dim={!live}
    title={live ? 'Thinking' : note}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={toggle}
  >
    Thinking
    {#if !on}<span class="state">off</span>{/if}
  </button>

  {#if open}
    <span class="pop" role="menu">
      <Toggle
        label="Thinking"
        hint="Set when the session starts"
        checked={on}
        onchange={(next) => {
          open = false;
          onchange?.(next);
        }}
      />
    </span>
  {/if}
</span>

<style>
  .thinking {
    position: relative;
    display: inline-flex;
  }

  /* The grey block of the original, quieter than the model beside it: it says
     one word and holds one switch. */
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: none;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-dim);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    padding: 5px 10px;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background 140ms ease-out,
      color 140ms ease-out;
  }

  .chip:hover:not(.dim) {
    background: rgba(255, 255, 255, 0.12);
    color: var(--text);
  }

  .chip.dim {
    cursor: default;
  }

  /* Off is a state worth seeing without opening anything. */
  .chip.off .state {
    color: var(--warn);
  }

  .chip:focus,
  .chip:focus-visible {
    outline: none;
  }

  .pop {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 0;
    z-index: 3;
    min-width: 240px;
    padding: 10px 12px;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    background: var(--notch);
    transform-origin: bottom left;
    animation: grow 160ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  @keyframes grow {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .pop {
      animation: none;
    }
  }
</style>
