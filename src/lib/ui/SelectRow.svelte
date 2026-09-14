<script lang="ts">
  import { ChevronDown } from '@lucide/svelte';

  type Option = { id: string; label: string; icon?: string };

  let {
    label,
    hint,
    options,
    value,
    busy = false,
    onchange,
  }: {
    label: string;
    hint?: string;
    options: Option[];
    value: string;
    /** The change has been sent and has not landed. */
    busy?: boolean;
    onchange?: (id: string) => void;
  } = $props();

  let open = $state(false);
  let host = $state<HTMLElement | null>(null);

  const current = $derived(options.find((option) => option.id === value) ?? null);

  function pick(id: string) {
    open = false;
    if (id !== value) onchange?.(id);
  }

  function keydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') {
      event.preventDefault();
      open = false;
    }
  }

  // A click anywhere else closes the list and goes on being that click. The
  // island's own dismissal ignores anything inside the shape. tech.md 6.7.
  function away(event: MouseEvent) {
    if (open && host && !host.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onkeydown={keydown} onclick={away} />

<!-- The layout of Toggle, with a value that opens a list where the switch
     would stand. tech.md 9. -->
<div class="row">
  <div class="words">
    <span class="label">{label}</span>
    {#if hint}
      <span class="hint">{hint}</span>
    {/if}
  </div>

  <div class="select" bind:this={host}>
    <button
      type="button"
      class="value"
      class:open
      aria-haspopup="listbox"
      aria-expanded={open}
      aria-label="{label}: {current?.label ?? ''}"
      aria-busy={busy}
      disabled={busy}
      onclick={() => (open = !open)}
    >
      {#if current?.icon}
        <span class="icon" aria-hidden="true">{current.icon}</span>
      {/if}
      <span>{current?.label ?? ''}</span>
      <span class="chevron" aria-hidden="true"><ChevronDown size={13} strokeWidth={2} /></span>
    </button>

    {#if open}
      <div class="menu" role="listbox" aria-label={label}>
        {#each options as option (option.id)}
          <button
            type="button"
            class="option"
            role="option"
            aria-selected={option.id === value}
            onclick={() => pick(option.id)}
          >
            {#if option.icon}
              <span class="icon" aria-hidden="true">{option.icon}</span>
            {/if}
            <span class="text">{option.label}</span>
            <span class="tick" aria-hidden="true">{option.id === value ? '✓' : ''}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 8px 2px;
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .label {
    color: var(--text);
    font-size: 13px;
    line-height: 1.3;
  }

  .hint {
    color: var(--text-dim);
    font-size: 11px;
    line-height: 1.35;
  }

  .select {
    position: relative;
    flex: none;
  }

  /* A fill, not an outline, brighter under the hand. tech.md 9. */
  .value {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    border: none;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    padding: 7px 10px 7px 11px;
    cursor: pointer;
    transition: background 140ms ease;
  }

  .value:hover:not(:disabled),
  .value.open {
    background: rgba(255, 255, 255, 0.15);
  }

  .value:disabled {
    cursor: default;
    color: var(--text-dim);
  }

  .value:focus,
  .value:focus-visible,
  .option:focus,
  .option:focus-visible {
    outline: none;
  }

  .chevron {
    display: flex;
    color: var(--text-dim);
    transition: transform 180ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .open .chevron {
    transform: rotate(180deg);
  }

  .icon {
    font-size: 15px;
    line-height: 1;
  }

  /* Grown from the button that opened it, downward: the settings stand at the
     top of the island with the room below them. Transform and opacity only.
     tech.md 6.10. */
  .menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 3;
    display: flex;
    flex-direction: column;
    min-width: 160px;
    padding: 4px;
    border-radius: 11px;
    background: #1c1c1f;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    transform-origin: top right;
    animation: grow 170ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  @keyframes grow {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .option {
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 9px;
    cursor: pointer;
  }

  .option:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .text {
    flex: 1;
    white-space: nowrap;
  }

  .tick {
    flex: none;
    width: 12px;
    color: var(--brand);
    font-size: 12px;
  }

  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }

    .chevron {
      transition: none;
    }
  }
</style>
