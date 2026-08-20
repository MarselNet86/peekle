<script lang="ts">
  import { blocks } from '$lib/logic/markdown';
  import type { FeedEntry } from '$lib/types/generated/FeedEntry';

  let { entry }: { entry: FeedEntry } = $props();

  const spoken = $derived(entry.kind === 'User' || entry.kind === 'Assistant');
  // Collapsed like the terminal shows it, opened by a click. tech.md 6.12.
  let open = $state(false);
  // Parsed into segments and rendered as elements. Never `{@html}`: this text
  // comes out of an agent turn into a window over the whole screen.
  const parts = $derived(spoken ? blocks(entry.text) : []);
</script>

<!-- What a person said and what the agent answered are messages: they wrap,
     they carry their whole text, and the user's own turn is the green one. A
     tool call stays a single quiet line. tech.md 9 and 6.12. -->
{#if spoken}
  <div class="line" data-kind={entry.kind}>
    <div class="bubble">
      {#each parts as block, index (index)}
        {#if block.kind === 'code'}
          <pre class="code"><code>{block.value}</code></pre>
        {:else}
          <p class="prose">
            {#each block.pieces as piece, at (at)}
              {#if piece.kind === 'code'}
                <code class="inline">{piece.value}</code>
              {:else if piece.kind === 'bold'}
                <strong>{piece.value}</strong>
              {:else}
                {piece.value}
              {/if}
            {/each}
          </p>
        {/if}
      {/each}
    </div>
  </div>
{:else}
  <div class="object" data-kind={entry.kind}>
    <button
      class="row"
      type="button"
      disabled={!entry.detail}
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <span class="dot" data-state={entry.state}></span>
      {#if entry.tool}
        <span class="tool">{entry.tool}</span>
      {/if}
      <span class="text">{entry.text}</span>
      {#if entry.detail}
        <span class="chevron" class:open aria-hidden="true">
          <svg viewBox="0 0 8 12" width="7" height="10">
            <path d="M1.5 1l5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
          </svg>
        </span>
      {/if}
    </button>
    {#if open && entry.detail}
      <pre class="detail">{entry.detail}</pre>
    {/if}
  </div>
{/if}

<style>
  .line {
    display: flex;
    padding: 3px 2px;
  }

  .bubble {
    max-width: 84%;
    padding: 7px 10px;
    border-radius: 12px;
    font-size: 13px;
    line-height: 1.45;
    /* The whole message, wrapped. Cutting it with an ellipsis was the reason
       the dialogue could not be read at all. */
    overflow-wrap: anywhere;
    min-width: 0;
  }

  /* The user's own turn, in the product's green, on the side a messenger puts
     it. tech.md 9. */
  .line[data-kind='User'] {
    justify-content: flex-end;
  }

  .line[data-kind='User'] .bubble {
    background: var(--brand);
    color: var(--notch);
    border-bottom-right-radius: 4px;
  }

  /* The agent's own ground. On pure black a translucent surface has no edge,
     and the message reads as loose text. tech.md 9. */
  .line[data-kind='Assistant'] .bubble {
    background: var(--bubble);
    border: 1px solid var(--hairline);
    color: var(--text);
    border-bottom-left-radius: 4px;
  }

  .prose {
    margin: 0;
    white-space: pre-wrap;
  }

  .prose + .prose,
  .prose + .code,
  .code + .prose {
    margin-top: 6px;
  }

  .inline,
  .code {
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 12px;
  }

  .inline {
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.35);
    color: var(--code);
  }

  .code {
    margin: 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.45);
    color: var(--code);
    overflow-x: auto;
    white-space: pre;
  }

  /* The user's own bubble is already green, so code inside it reads by weight
     rather than by colour. */
  .line[data-kind='User'] .inline,
  .line[data-kind='User'] .code {
    background: rgba(0, 0, 0, 0.18);
    color: var(--notch);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: var(--row);
    padding: 0 4px;
    border: none;
    background: transparent;
    font: inherit;
    text-align: left;
    min-width: 0;
    cursor: pointer;
  }

  .row:disabled {
    cursor: default;
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  .row:focus,
  .row:focus-visible {
    outline: none;
  }

  .row:hover:not(:disabled) .text {
    color: var(--text);
  }

  .chevron {
    flex: none;
    color: var(--text-dim);
    display: flex;
    transition: transform 120ms ease-out;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  /* The body of an object, as the terminal prints it. */
  .detail {
    margin: 0 4px 6px 18px;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.45);
    border: 1px solid var(--hairline);
    color: var(--text-dim);
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    line-height: 1.5;
    max-height: 220px;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  /* Reasoning is a marker, so it stays out of the way until asked for. */
  .object[data-kind='Thought'] .text {
    font-style: italic;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  /* A call still in flight, one that reported success, and one that never
     reported at all by the time the turn ended. tech.md 6.3. */
  .dot[data-state='Running'] {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }
  .dot[data-state='Ok'] {
    background: transparent;
    border: 1px solid var(--text-dim);
  }
  .dot[data-state='Failed'] {
    background: var(--danger);
  }

  /* The tool name is an identifier, so it reads as one. */
  .tool {
    flex: none;
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    color: var(--text-dim);
  }

  .text {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
