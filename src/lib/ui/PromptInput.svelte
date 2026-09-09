<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    value = $bindable(''),
    placeholder = '',
    disabled = false,
    working = false,
    tools,
    onsubmit,
    onstop,
    onescape,
  }: {
    value?: string;
    placeholder?: string;
    disabled?: boolean;
    /** A turn is running and can be ended from here. The one button turns
     * into the way to end it, which is where Claude Code puts it too: no
     * second control appears, and none has to be found. tech.md 6.5. */
    working?: boolean;
    /** What sits in the row under the text, on the left of the send button:
     * the controls of the message being written. Claude Code's own composer
     * is laid out this way, and for the same reason. tech.md 6.15. */
    tools?: Snippet;
    onsubmit?: (text: string) => void;
    onstop?: () => void;
    onescape?: () => void;
  } = $props();

  let field: HTMLTextAreaElement | undefined = $state();

  $effect(() => {
    field?.focus();
  });

  // Nothing to send is nothing to press. A button that does nothing when
  // clicked lies about its own state. tech.md 9.
  const sendable = $derived(!disabled && value.trim().length > 0);
  // While a turn runs the button ends it, whatever the field holds. Typing
  // still sends: Enter queues the next message the way the terminal does, and
  // the button is the only thing that changes. tech.md 6.5.
  const stops = $derived(working);

  function send() {
    if (sendable) onsubmit?.(value);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onescape?.();
      return;
    }
    // Enter sends, Shift+Enter breaks the line. The panel is a reply box, not
    // an editor, so sending is the common case.
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      onsubmit?.(value);
    }
  }
</script>

<!-- One capsule: the text on top, its controls under it. tech.md 6.15. -->
<div class="capsule" class:off={disabled}>
  <textarea
    bind:this={field}
    bind:value
    {placeholder}
    {disabled}
    rows="1"
    spellcheck="false"
    onkeydown={keydown}></textarea>

  <div class="tools">
    <div class="left">
      {#if tools}{@render tools()}{/if}
    </div>
    <!-- mousedown is swallowed so the caret stays where the user left it:
         pressing send must not take the field's focus away. tech.md 6.7. -->
    <button
      class="send"
      class:stop={stops}
      type="button"
      disabled={stops ? false : !sendable}
      aria-label={stops ? 'Stop' : 'Send'}
      onmousedown={(event) => event.preventDefault()}
      onclick={() => (stops ? onstop?.() : send())}
    >
      {#if stops}
        <!-- The square everything else uses for stop, filled and centred. -->
        <svg viewBox="0 0 14 14" width="14" height="14" aria-hidden="true">
          <rect x="4" y="4" width="6" height="6" rx="1.2" fill="currentColor" />
        </svg>
      {:else}
        <svg viewBox="0 0 14 14" width="14" height="14" aria-hidden="true">
          <path
            d="M7 11.5V2.5M7 2.5L3 6.5M7 2.5l4 4"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      {/if}
    </button>
  </div>
</div>

<style>
  .capsule {
    min-width: 0;
    border: 1px solid var(--hairline);
    border-radius: 20px;
    padding: 9px 9px 9px 16px;
    transition: border-color 120ms ease;
  }

  /* The controls of the message being written, under it. tech.md 6.15. */
  .tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 5px;
  }

  .left {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    /* Fills the row, so what the row puts at its right edge lands next to the
       send button rather than in the middle of nothing. tech.md 6.15. */
    flex: 1;
    /* The first control's own padding pulled back, so its label starts on the
       same line the text above it starts on. */
    margin-left: -6px;
  }

  .capsule:focus-within {
    border-color: rgba(255, 255, 255, 0.18);
  }

  .capsule.off {
    opacity: 0.5;
  }

  textarea {
    display: block;
    width: 100%;
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 15px;
    line-height: 22px;
    padding: 0;
    field-sizing: content;
    max-height: 132px;
  }

  textarea::placeholder {
    color: var(--text-dim);
  }

  /* Stopping is the accent doing something, so it wears the accent: a white
     square on the product's own green. It is never dimmed -- there is always
     a turn to end while it is shown. tech.md 9. */
  .send.stop,
  .send.stop:disabled {
    background: var(--brand);
    color: var(--text);
    opacity: 1;
  }

  .send {
    flex: none;
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: none;
    border-radius: 999px;
    background: var(--bubble);
    color: var(--text-dim);
    cursor: pointer;
    transition:
      color 120ms ease,
      background 120ms ease,
      opacity 120ms ease;
  }

  .send:not(:disabled):hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.12);
  }

  .send:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
