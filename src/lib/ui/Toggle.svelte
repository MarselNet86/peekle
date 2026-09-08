<script lang="ts">
  let {
    label,
    checked,
    hint,
    busy = false,
    onchange,
  }: {
    label: string;
    checked: boolean;
    /** The second line, for what the switch cannot promise on its own. */
    hint?: string;
    /** Waiting on the answer to the last flick. A switch that moves before the
     * answer lands is a switch that lies about the state. tech.md 9. */
    busy?: boolean;
    onchange?: (next: boolean) => void;
  } = $props();
</script>

<!-- A whole row of settings, not a bare switch: the label is what the switch
     means, and splitting them leaves each feature to lay the pair out again.
     tech.md 9. -->
<div class="row">
  <div class="words">
    <span class="label">{label}</span>
    {#if hint}
      <span class="hint">{hint}</span>
    {/if}
  </div>
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    aria-busy={busy}
    disabled={busy}
    class:on={checked}
    onclick={() => onchange?.(!checked)}
  >
    <span class="knob"></span>
  </button>
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

  button {
    flex: none;
    position: relative;
    width: 36px;
    height: 21px;
    margin-top: 1px;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--bubble);
    cursor: pointer;
    transition:
      background 140ms ease,
      border-color 140ms ease;
  }

  button:disabled {
    cursor: default;
  }

  /* The accent means on, the same green the product uses for its own sign. */
  .on {
    background: var(--brand);
    border-color: var(--brand);
  }

  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--text);
    transition: transform 140ms ease;
  }

  .on .knob {
    background: var(--notch);
    transform: translateX(15px);
  }
</style>
