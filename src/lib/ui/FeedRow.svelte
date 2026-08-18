<script lang="ts">
  import type { FeedEntry } from '$lib/types/generated/FeedEntry';
  import MessageBlock from './MessageBlock.svelte';

  let { entry }: { entry: FeedEntry } = $props();

  // What the agent said is prose and wants room; everything else is one line.
  // tech.md S6.
  let collapsed = $state(true);
</script>

{#if entry.kind === 'Assistant'}
  <div class="row block" data-kind={entry.kind}>
    <span class="dot" data-state={entry.state}></span>
    <MessageBlock text={entry.text} bind:collapsed />
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
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* The user's own turn is the anchor of the feed, so it carries full weight
     while the tool calls under it stay quiet. */
  .row[data-kind='User'] .text {
    color: var(--text);
    font-weight: 500;
  }

  .row[data-kind='Tool'] .text {
    color: var(--text-dim);
  }

  /* Prose needs its own height, so this row stops pretending to be a line. */
  .row.block {
    height: auto;
    align-items: flex-start;
    padding-top: 8px;
    padding-bottom: 8px;
  }

  .row.block .dot {
    margin-top: 6px;
  }
</style>
