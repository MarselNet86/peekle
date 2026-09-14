<script lang="ts">
  import { COMMON } from '$lib/i18n/common';
  import { copy } from '$lib/i18n/index.svelte';
  import Button from './Button.svelte';

  let {
    command,
    copied = false,
    oncopy,
  }: {
    command: string;
    /** The last press copied it. The label says so for two seconds. */
    copied?: boolean;
    oncopy?: () => void;
  } = $props();

  const t = $derived(copy(COMMON));
</script>

<!-- A line of a terminal, not a paragraph: the command scrolls sideways
     rather than wrapping, because a command broken across two lines is copied
     by hand as two commands. tech.md 9 and 6.16. -->
<div class="command">
  <span class="prompt" aria-hidden="true">$</span>
  <code class="line">{command}</code>
  <!-- Quiet in both states, the way a macOS copy button answers: the word
       changes and nothing lights up. A green fill for `Copied` was louder
       than the command it copied. -->
  <Button label={copied ? t.copied : t.copy} onclick={() => oncopy?.()} />
</div>

<style>
  .command {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    box-sizing: border-box;
    padding: 5px 5px 5px 10px;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--bubble);
  }

  .prompt {
    flex: none;
    font-family: ui-monospace, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    color: var(--text-dim);
    opacity: 0.6;
  }

  .line {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    scrollbar-width: none;
    font-family: ui-monospace, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    color: var(--text);
    text-align: left;
    /* The command is the one thing on this line a person may want to take by
       hand, so it stays selectable where the overlay is not. tech.md 9. */
    user-select: text;
    -webkit-user-select: text;
  }

  .line::-webkit-scrollbar {
    display: none;
  }

  /* One width for `Copy` and `Copied`, so the word changing does not take
     width from the command beside it. */
  .command :global(button) {
    min-width: 64px;
    padding: 7px 10px;
  }
</style>
