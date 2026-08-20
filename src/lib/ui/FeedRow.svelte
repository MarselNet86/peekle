<script lang="ts">
  import type { FeedEntry } from '$lib/types/generated/FeedEntry';

  let { entry }: { entry: FeedEntry } = $props();

  const spoken = $derived(entry.kind === 'User' || entry.kind === 'Assistant');
</script>

<!-- What a person said and what the agent answered are messages: they wrap,
     they carry their whole text, and the user's own turn is the green one. A
     tool call stays a single quiet line. tech.md 9 and 6.12. -->
{#if spoken}
  <div class="line" data-kind={entry.kind}>
    <div class="bubble">{entry.text}</div>
  </div>
{:else}
  <div class="row" data-kind={entry.kind}>
    <span class="dot" data-state={entry.state}></span>
    {#if entry.tool}
      <span class="tool">{entry.tool}</span>
    {/if}
    <span class="text">{entry.text}</span>
  </div>
{/if}

<style>
  .line {
    display: flex;
    padding: 3px 2px;
  }

  .bubble {
    max-width: 82%;
    padding: 7px 10px;
    border-radius: 12px;
    font-size: 13px;
    line-height: 1.45;
    /* The whole message, wrapped. Cutting it with an ellipsis was the reason
       the dialogue could not be read at all. */
    white-space: pre-wrap;
    overflow-wrap: anywhere;
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

  .line[data-kind='Assistant'] .bubble {
    background: var(--surface);
    color: var(--text);
    border-bottom-left-radius: 4px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--row);
    padding: 0 4px;
    min-width: 0;
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
