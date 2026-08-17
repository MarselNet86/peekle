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

<textarea
  bind:this={field}
  bind:value
  {placeholder}
  {disabled}
  rows="1"
  spellcheck="false"
  onkeydown={keydown}></textarea>

<style>
  textarea {
    width: 100%;
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 15px;
    line-height: 22px;
    padding: 6px 0;
    field-sizing: content;
    max-height: 132px;
  }

  textarea::placeholder {
    color: var(--text-dim);
  }

  textarea:disabled {
    opacity: 0.5;
  }
</style>
