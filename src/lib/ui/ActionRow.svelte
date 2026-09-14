<script lang="ts">
  import Button from './Button.svelte';

  let {
    label,
    hint,
    action,
    confirm,
    busy = false,
    error = null,
    onaction,
  }: {
    label: string;
    /** The second line while nothing is being asked. */
    hint?: string;
    /** The button's word. */
    action: string;
    /** The second line while the row asks, saying what the second press does. */
    confirm: string;
    busy?: boolean;
    /** Why the last press did not go through, in words. */
    error?: string | null;
    /** Called by the second press only. tech.md 6.16. */
    onaction?: () => void;
  } = $props();

  /** How long the question stands before it takes itself back: long enough
   * to read, short enough that a row left alone is never found asking. The
   * same as the bin in the session list. tech.md 6.26. */
  const CONFIRM_FOR = 4000;

  let asking = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function forget() {
    clearTimeout(timer);
    asking = false;
  }

  function press() {
    if (asking) {
      forget();
      onaction?.();
      return;
    }
    asking = true;
    clearTimeout(timer);
    timer = setTimeout(() => (asking = false), CONFIRM_FOR);
  }

  $effect(() => () => clearTimeout(timer));
</script>

<!-- A row of settings that does something rather than switches something:
     the same layout as `Toggle`, a button where the switch would be, and a
     second press for anything one press must not do. tech.md 9. -->
<div
  class="row"
  class:asking
  role="group"
  aria-label={label}
  onpointerleave={() => {
    if (!busy) forget();
  }}
>
  <div class="words">
    <span class="label">{label}</span>
    {#if error}
      <span class="hint fault">{error}</span>
    {:else if asking}
      <span class="hint warn">{confirm}</span>
    {:else if hint}
      <span class="hint">{hint}</span>
    {/if}
  </div>
  <div class="buttons">
    {#if asking && !busy}
      <Button label="Cancel" onclick={forget} />
    {/if}
    <Button label={action} variant={asking || busy ? 'danger' : 'ghost'} {busy} onclick={press} />
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 8px 2px;
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .label {
    color: var(--text);
    font-size: 13px;
    line-height: 1.3;
  }

  .hint {
    color: var(--text-dim);
    font-size: 11px;
    line-height: 1.35;
  }

  .hint.warn {
    color: var(--warn);
  }

  .hint.fault {
    color: var(--danger);
  }

  .buttons {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
