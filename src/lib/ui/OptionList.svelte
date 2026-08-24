<script lang="ts">
  // bits-ui 2 ships no Listbox. RadioGroup is its headless single-select with
  // roving focus, which is exactly the keyboard behaviour section 9 asks for.
  import { RadioGroup } from 'bits-ui';
  import { parseLabel } from '$lib/logic/options';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';

  let {
    options,
    selected = $bindable(''),
    onselect,
  }: {
    options: ChoiceOption[];
    selected?: string;
    onselect?: (id: string) => void;
  } = $props();

  function keydown(event: KeyboardEvent) {
    // Digits pick a row outright. Four options at most, so 1..4 covers it.
    const digit = Number(event.key);
    if (!Number.isInteger(digit) || digit < 1 || digit > options.length) return;
    event.preventDefault();
    const option = options[digit - 1];
    selected = option.id;
    onselect?.(option.id);
  }
</script>

<svelte:window onkeydown={keydown} />

<RadioGroup.Root bind:value={selected} onValueChange={(id: string) => onselect?.(id)} class="list">
  {#each options as option, index (option.id)}
    <RadioGroup.Item value={option.id} class="row" data-kind={option.kind}>
      <span class="index">{index + 1}</span>
      <span class="label">{parseLabel(option.label).text}</span>
      <!-- Claude's own pick, marked the way a recommended AskUserQuestion
           answer already is: a trailing "(Recommended)" in the label.
           tech.md 9. -->
      {#if parseLabel(option.label).recommended}
        <span class="recommended">Recommended</span>
      {/if}
      {#if option.hint}
        <span class="hint">{option.hint}</span>
      {/if}
    </RadioGroup.Item>
  {/each}
</RadioGroup.Root>

<style>
  :global(.list) {
    display: flex;
    flex-direction: column;
    gap: 2px;
    outline: none;
  }

  :global(.row) {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: var(--row);
    padding: 0 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: default;
  }

  :global(.row:focus-visible),
  :global(.row[data-state='checked']) {
    background: rgba(255, 255, 255, 0.08);
    outline: none;
  }

  :global(.row[data-kind='Deny'][data-state='checked']) {
    background: rgba(232, 101, 74, 0.18);
  }

  .index {
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    width: 12px;
  }

  .label {
    font-size: 14px;
  }

  .hint {
    margin-left: auto;
    color: var(--text-dim);
    font-size: 11px;
  }

  .recommended {
    margin-left: auto;
    flex: none;
    padding: 1px 6px;
    border-radius: 999px;
    background: rgba(125, 216, 143, 0.16);
    color: var(--accent);
    font-size: 10px;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }
</style>
