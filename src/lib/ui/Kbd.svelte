<script lang="ts">
  let {
    keys,
    cap = false,
  }: {
    keys: string[];
    /** Drawn as a key rather than mentioned as one: a raised cap the size of
     * a real keycap, for the one place where pressing it is the whole point
     * of the pill. tech.md 6.13 and 9. */
    cap?: boolean;
  } = $props();
</script>

<span class="keys" class:cap>
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

  /* A key you are being asked to press, drawn the size of the key itself:
     seen without being read, which a 10px outline never was. The bottom edge
     is heavier, the way a keycap catches light on this system. */
  .cap kbd {
    display: grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    padding: 0 6px;
    font-size: 14px;
    line-height: 1;
    border-radius: 7px;
    border-color: rgba(255, 255, 255, 0.22);
    border-bottom-width: 2px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text);
  }
</style>
