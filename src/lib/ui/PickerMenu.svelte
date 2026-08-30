<script lang="ts">
  /**
   * A short menu over the content: the current value as a button, the options
   * above it, a tick on the one in force. tech.md 9 and 6.15.
   *
   * It opens upward because it lives in the bottom strip of the island, and a
   * menu that opens down there is a menu drawn off the shape.
   */
  import type { PickOption } from '$lib/logic/agent';

  let {
    label,
    options,
    value = '',
    disabled = false,
    pending = false,
    onpick,
  }: {
    /** What the button says when nothing is picked yet. */
    label: string;
    options: PickOption[];
    value?: string;
    disabled?: boolean;
    /** The pick has been sent and the agent has not confirmed it. 6.15. */
    pending?: boolean;
    onpick: (id: string) => void;
  } = $props();

  let open = $state(false);
  let host = $state<HTMLElement | null>(null);

  const usable = $derived(!disabled && options.length > 0);

  function pick(id: string) {
    open = false;
    onpick(id);
  }

  function toggle() {
    if (!usable) return;
    open = !open;
  }

  // Digits and Escape, the same keys OptionList answers to. The island does
  // not hold the keyboard until the user clicks into it, so these are a second
  // way in rather than the only one. tech.md 9.
  function keydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      open = false;
      return;
    }
    const digit = Number(event.key);
    if (!Number.isInteger(digit) || digit < 1 || digit > options.length) return;
    event.preventDefault();
    pick(options[digit - 1].id);
  }

  // A click anywhere else closes the menu and goes on being that click. The
  // island's own dismissal already ignores anything inside the shape, so this
  // never fights it. tech.md 6.7.
  function away(event: MouseEvent) {
    if (!open || !host) return;
    if (!host.contains(event.target as Node)) open = false;
  }
</script>

<svelte:window onkeydown={keydown} onclick={away} />

<span class="picker-menu" bind:this={host}>
  <button
    class="value"
    class:pending
    disabled={!usable}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={toggle}
  >
    {label}
  </button>

  {#if open}
    <span class="menu" role="menu">
      {#each options as option, index (option.id)}
        <button
          class="option"
          role="menuitemradio"
          aria-checked={option.id === value}
          onclick={() => pick(option.id)}
        >
          <span class="tick" aria-hidden="true">{option.id === value ? '✓' : ''}</span>
          <span class="text">{option.label}</span>
          <span class="index">{index + 1}</span>
        </button>
      {/each}
    </span>
  {/if}
</span>

<style>
  .picker-menu {
    position: relative;
    display: inline-flex;
  }

  .value {
    border: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    padding: 3px 6px;
    border-radius: 999px;
    cursor: pointer;
    white-space: nowrap;
  }

  .value:hover:not(:disabled) {
    background: var(--bubble);
  }

  .value:disabled {
    color: var(--text-dim);
    cursor: default;
  }

  /* Sent, not yet confirmed: the agent names its own model on its next turn,
     and until then the row says what was asked for, quietly. tech.md 6.15. */
  .value.pending {
    color: var(--text-dim);
  }

  .menu {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    z-index: 3;
    display: flex;
    flex-direction: column;
    min-width: 130px;
    padding: 4px;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--notch);
  }

  .option {
    display: flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 12px;
    text-align: left;
    padding: 5px 6px;
    border-radius: 7px;
    cursor: pointer;
  }

  .option:hover {
    background: var(--bubble);
  }

  .tick {
    flex: none;
    width: 10px;
    color: var(--brand);
    font-size: 10px;
  }

  .text {
    flex: 1;
    white-space: nowrap;
  }

  .index {
    flex: none;
    color: var(--text-dim);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }
</style>
