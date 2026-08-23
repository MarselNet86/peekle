<script lang="ts">
  let {
    value = $bindable(''),
    placeholder = '',
    disabled = false,
    onsubmit,
    onescape,
  }: {
    value?: string;
    placeholder?: string;
    disabled?: boolean;
    onsubmit?: (text: string) => void;
    onescape?: () => void;
  } = $props();

  let field: HTMLTextAreaElement | undefined = $state();

  $effect(() => {
    field?.focus();
  });

  // Nothing to send is nothing to press. A button that does nothing when
  // clicked lies about its own state. tech.md 9.
  const sendable = $derived(!disabled && value.trim().length > 0);

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

<div class="row">
  <div class="capsule" class:off={disabled}>
    <textarea
      bind:this={field}
      bind:value
      {placeholder}
      {disabled}
      rows="1"
      spellcheck="false"
      onkeydown={keydown}></textarea>
  </div>
  <!-- mousedown is swallowed so the caret stays where the user left it:
       pressing send must not take the field's focus away. tech.md 6.7. -->
  <button
    class="send"
    type="button"
    disabled={!sendable}
    aria-label="Send"
    onmousedown={(event) => event.preventDefault()}
    onclick={send}
  >
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
  </button>
</div>

<style>
  .row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }

  .capsule {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--hairline);
    /* Exactly half the one-line height, so a single line is a true pill and a
       grown field keeps the same corner. */
    border-radius: 18px;
    padding: 6px 16px;
    transition: border-color 120ms ease;
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

  .send {
    flex: none;
    width: 30px;
    height: 30px;
    margin-bottom: 2px;
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
