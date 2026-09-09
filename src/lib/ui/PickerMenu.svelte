<script lang="ts">
  /**
   * A short menu over the content: the current value as a button, the options
   * above it, a tick on the one in force. tech.md 9 and 6.15.
   *
   * It opens upward because it lives in the bottom strip of the island, and a
   * menu that opens down there is a menu drawn off the shape.
   */
  import { CodeXml, Hand, ScrollText, Zap } from '@lucide/svelte';

  import type { PickIcon, PickOption } from '$lib/logic/agent';

  /** The signs the rows wear. Names are the icon set's own: a hand drawn by
   * hand comes out a blob. tech.md 9. */
  const SIGNS = { hand: Hand, code: CodeXml, plan: ScrollText, bolt: Zap };

  let {
    label,
    options,
    value = '',
    icon,
    disabled = false,
    pending = false,
    onpick,
  }: {
    /** What the button says when nothing is picked yet. */
    label: string;
    options: PickOption[];
    /** The sign the button wears. It is what tells a value that opens a menu
     * from a value that only reads, now that the chevron is gone: a sign is
     * seen without being read, which a small triangle never was. tech.md 9. */
    icon?: PickIcon;
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
    <!-- The sign says this opens something, which is what the chevron used
         to say and said badly: a triangle at eight pixels is noise, a hand
         and a bolt are read at a glance. Hidden from the accessible name --
         the button is still called by its value. tech.md 9. -->
    {#if icon}
      {@const Sign = SIGNS[icon]}
      <Sign size={13} strokeWidth={1.75} aria-hidden="true" />
    {/if}
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
          {#if option.icon}
            {@const Sign = SIGNS[option.icon]}
            <span class="sign"><Sign size={15} strokeWidth={1.6} aria-hidden="true" /></span>
          {/if}
          <!-- A row that needs explaining says so under itself, the way the
               original's own mode menu does. tech.md 6.19. -->
          <span class="text">
            {option.label}
            {#if option.hint}
              <span class="hint">{option.hint}</span>
            {/if}
          </span>
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
    display: inline-flex;
    align-items: center;
    gap: 5px;
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

  /* Grown from the button that opened it, not swapped in for it: a menu that
     appears whole and instantly reads as a change of frame. Transform and
     opacity only -- nothing here relays out the island. tech.md 6.10. */
  .menu {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    z-index: 3;
    transform-origin: bottom left;
    animation: grow 160ms cubic-bezier(0.22, 1, 0.36, 1);
    display: flex;
    flex-direction: column;
    min-width: 130px;
    padding: 4px;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--notch);
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
    .menu {
      animation: none;
    }
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

  .hint {
    display: block;
    margin-top: 2px;
    font-size: 10px;
    line-height: 1.35;
    color: var(--text-dim);
    white-space: normal;
  }

  /* A row with a line under it is two lines tall, so the tick and the number
     stand at its top rather than floating in the middle of it. */
  .option:has(.hint) {
    align-items: flex-start;
  }

  .option:has(.hint) .tick,
  .option:has(.hint) .index {
    margin-top: 1px;
  }

  /* Descriptions need room the value menus never did, and the room has to be
     taken rather than offered: a menu sized by its longest word wraps every
     line of every hint. tech.md 6.19. */
  .menu:has(.hint) {
    min-width: 300px;
  }

  .sign {
    flex: none;
    display: flex;
    color: var(--text-dim);
    margin-top: 1px;
  }

  .option[aria-checked='true'] .sign {
    color: var(--text);
  }

  .index {
    flex: none;
    color: var(--text-dim);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }
</style>
