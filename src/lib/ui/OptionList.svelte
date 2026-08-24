<script lang="ts">
  // bits-ui 2 ships no Listbox. RadioGroup is its headless single-select with
  // roving focus, which is exactly the keyboard behaviour section 9 asks for.
  // `multiple` switches to Checkbox.Group for the same reason: a permission
  // is one choice, an AskUserQuestion multiSelect question is several, and
  // forcing the second through a single-select control would silently drop
  // every pick but the last. tech.md 6.14.
  import { Checkbox, RadioGroup } from 'bits-ui';
  import { parseLabel } from '$lib/logic/options';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';

  let {
    options,
    selected = $bindable(''),
    onselect,
    multiple = false,
    values = $bindable<string[]>([]),
  }: {
    options: ChoiceOption[];
    selected?: string;
    onselect?: (id: string) => void;
    /** Checkboxes instead of radio: more than one may be chosen at once, and
     * nothing fires until whatever asked for the answer collects it itself.
     * The AskUserQuestion case this exists for answers several questions at
     * once, so a submit that belonged to one row's own list would be the
     * wrong scope. tech.md 6.14. */
    multiple?: boolean;
    values?: string[];
  } = $props();

  function toggle(id: string) {
    values = values.includes(id) ? values.filter((v) => v !== id) : [...values, id];
  }

  function keydown(event: KeyboardEvent) {
    // Digits pick a row outright in single mode, toggle it in multi mode.
    // Four options at most, so 1..4 covers it.
    const digit = Number(event.key);
    if (!Number.isInteger(digit) || digit < 1 || digit > options.length) return;
    event.preventDefault();
    const option = options[digit - 1];
    if (multiple) {
      toggle(option.id);
      return;
    }
    selected = option.id;
    onselect?.(option.id);
  }
</script>

<svelte:window onkeydown={keydown} />

{#if multiple}
  <Checkbox.Group bind:value={values} class="list">
    {#each options as option, index (option.id)}
      <Checkbox.Root value={option.id} class="row" data-kind={option.kind}>
        {#snippet children({ checked }: { checked: boolean })}
          <span class="box" class:checked aria-hidden="true">
            {#if checked}
              <svg viewBox="0 0 10 8" width="10" height="8">
                <path
                  d="M1 4l2.5 2.5L9 1"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.6"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            {/if}
          </span>
          <span class="index">{index + 1}</span>
          <span class="label">{parseLabel(option.label).text}</span>
          {#if parseLabel(option.label).recommended}
            <span class="recommended">Recommended</span>
          {/if}
          {#if option.hint}
            <span class="hint">{option.hint}</span>
          {/if}
        {/snippet}
      </Checkbox.Root>
    {/each}
  </Checkbox.Group>
{:else}
  <RadioGroup.Root
    bind:value={selected}
    onValueChange={(id: string) => onselect?.(id)}
    class="list"
  >
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
{/if}

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

  .box {
    display: grid;
    place-items: center;
    flex: none;
    width: 14px;
    height: 14px;
    border: 1px solid var(--hairline);
    border-radius: 4px;
    color: var(--notch);
  }

  .box.checked {
    background: var(--accent);
    border-color: var(--accent);
  }
</style>
