<script lang="ts">
  let {
    label,
    value,
    hint,
  }: {
    label: string;
    /** What the row says, on the right. */
    value: string;
    /** The second line, under the label. */
    hint?: string;
  } = $props();
</script>

<!-- A row of settings that says something and asks nothing: the layout of
     Toggle, with a value where the control would stand. The value can be
     selected, because it is what a person copies into a bug report.
     tech.md 9. -->
<div class="row">
  <div class="words">
    <span class="label">{label}</span>
    {#if hint}
      <span class="hint">{hint}</span>
    {/if}
  </div>
  <span class="value">{value}</span>
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

  .value {
    flex: none;
    color: var(--text-dim);
    font-size: 13px;
    line-height: 1.3;
    font-variant-numeric: tabular-nums;
    user-select: text;
    cursor: text;
  }
</style>
