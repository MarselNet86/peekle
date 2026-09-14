<script lang="ts">
  // bits-ui 2 ships no Listbox. RadioGroup is its headless single-select with
  // roving focus, which is exactly the keyboard behaviour section 9 asks for.
  // `multiple` switches to Checkbox.Group for the same reason: a permission
  // is one choice, an AskUserQuestion multiSelect question is several, and
  // forcing the second through a single-select control would silently drop
  // every pick but the last. tech.md 6.14.
  import { Checkbox, RadioGroup } from 'bits-ui';
  import { CHAT } from '$lib/i18n/chat';
  import { copy } from '$lib/i18n/index.svelte';
  import { parseLabel } from '$lib/logic/options';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';

  const t = $derived(copy(CHAT));

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

  /** Whether the key went into something that takes text. */
  function typing(target: EventTarget | null): boolean {
    const element = target as HTMLElement | null;
    if (!element) return false;
    return (
      element.isContentEditable || element.tagName === 'INPUT' || element.tagName === 'TEXTAREA'
    );
  }

  /**
   * Picks a row by its number, counted from one: outright in single mode,
   * toggled in multi mode. A number with no row behind it does nothing.
   *
   * The route calls this for ⌘ and a digit pressed in another application,
   * which reaches the island as an event rather than a keystroke. tech.md 6.14.
   */
  export function pick(index: number): boolean {
    const option = options[index - 1];
    if (!Number.isInteger(index) || !option) return false;
    if (multiple) {
      toggle(option.id);
      return true;
    }
    selected = option.id;
    onselect?.(option.id);
    return true;
  }

  function keydown(event: KeyboardEvent) {
    if (event.ctrlKey || event.altKey) return;
    // A digit typed into a field is a digit, not a shortcut. The list listens
    // on the window, so without this it swallows every number written in the
    // answer field standing beside it. ⌘ and a digit is never text, so it
    // picks from inside the field as well. tech.md 6.14.
    if (typing(event.target) && !event.metaKey) return;

    const digit = Number(event.key);
    if (!Number.isInteger(digit) || digit < 1 || digit > options.length) return;
    event.preventDefault();
    pick(digit);
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- An answer label runs as long as Claude wrote it, so a row is a stack that
     grows down rather than a fixed line that overlaps the next one. -->
{#snippet body(option: ChoiceOption, index: number)}
  <!-- The key that picks the row, drawn as the key: ⌘ and its digit. tech.md
       6.14. -->
  <span class="key" aria-hidden="true"><span class="cmd">⌘</span><span>{index + 1}</span></span>
  <span class="text">
    <span class="line">
      <span class="label">{parseLabel(option.label).text}</span>
      <!-- Claude's own pick, marked the way a recommended AskUserQuestion
           answer already is: a trailing "(Recommended)" in the label. -->
      {#if parseLabel(option.label).recommended}
        <span class="recommended">{t.recommended}</span>
      {/if}
    </span>
    {#if option.hint}
      <span class="hint">{option.hint}</span>
    {/if}
  </span>
{/snippet}

{#if multiple}
  <Checkbox.Group bind:value={values} class="option-list">
    {#each options as option, index (option.id)}
      <Checkbox.Root value={option.id} class="option" data-kind={option.kind}>
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
          {@render body(option, index)}
        {/snippet}
      </Checkbox.Root>
    {/each}
  </Checkbox.Group>
{:else}
  <RadioGroup.Root
    bind:value={selected}
    onValueChange={(id: string) => onselect?.(id)}
    class="option-list"
  >
    {#each options as option, index (option.id)}
      <RadioGroup.Item value={option.id} class="option" data-kind={option.kind}>
        {@render body(option, index)}
      </RadioGroup.Item>
    {/each}
  </RadioGroup.Root>
{/if}

<style>
  :global(.option-list) {
    display: flex;
    flex-direction: column;
    gap: 6px;
    outline: none;
  }

  /* A card per answer: a row reads as pressable before the pointer reaches
     it, and the key that picks it stands at its head. The ground is a breath
     of the brand green rather than grey, so the rows belong to the product
     and not to a system dialog. tech.md 9. */
  :global(.option) {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 44px;
    padding: 8px 12px 8px 8px;
    border: 1px solid rgba(48, 209, 88, 0.1);
    border-radius: 10px;
    background: rgba(48, 209, 88, 0.05);
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      background 140ms ease,
      border-color 140ms ease,
      box-shadow 140ms ease;
  }

  /* Lighter than the checked state on purpose: hover says "this one is under
     the pointer", checked says "this one is the answer", and they are on
     screen together. */
  :global(.option:hover) {
    background: rgba(48, 209, 88, 0.1);
    border-color: rgba(48, 209, 88, 0.24);
  }

  :global(.option:focus-visible),
  :global(.option[data-state='checked']) {
    background: rgba(48, 209, 88, 0.16);
    border-color: rgba(48, 209, 88, 0.5);
    box-shadow: 0 0 20px rgba(48, 209, 88, 0.14);
    outline: none;
  }

  :global(.option[data-kind='Deny'][data-state='checked']) {
    background: rgba(232, 101, 74, 0.18);
    border-color: rgba(232, 101, 74, 0.5);
    box-shadow: none;
  }

  .key {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 1px;
    min-width: 30px;
    height: 28px;
    padding: 0 6px;
    border-radius: 7px;
    background: rgba(48, 209, 88, 0.14);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    transition: background 140ms ease;
  }

  .cmd {
    font-size: 11px;
    opacity: 0.85;
  }

  :global(.option:hover) .key,
  :global(.option[data-state='checked']) .key {
    background: rgba(48, 209, 88, 0.26);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .line {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .label {
    font-size: 13px;
    line-height: 18px;
  }

  .hint {
    color: var(--text-dim);
    font-size: 11px;
    line-height: 16px;
  }

  .recommended {
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
