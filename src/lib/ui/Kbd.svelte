<script lang="ts">
  let { keys }: { keys: string[] } = $props();
</script>

<span class="keys">
  {#each keys as key (key)}
    <kbd>{key}</kbd>
  {/each}
</span>

<style>
  .keys {
    display: inline-flex;
    gap: 3px;
  }

  kbd {
    font: inherit;
    font-size: 10px;
    line-height: 14px;
    padding: 0 4px;
    border-radius: 4px;
    border: 1px solid var(--hairline);
    color: var(--text-dim);
  }
</style>
