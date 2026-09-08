<script lang="ts">
  /**
   * An answer to a press that cannot do what was asked: what is so, and what
   * to do about it. tech.md 9 and 6.15.
   *
   * Built as an answer rather than as a failure. A single dim line at the
   * width of the island reads as an error string, and the reader takes it as
   * something broken instead of something explained. So the fact stands in
   * the reading colour on the bubble the feed already uses, and the way out
   * sits under it, quieter: it is only worth reading if the fact is
   * unwelcome.
   */
  let { fact, how = null }: { fact: string; how?: string | null } = $props();
</script>

<p class="note">
  <span class="fact">{fact}</span>
  {#if how}<span class="how">{how}</span>{/if}
</p>

<style>
  .note {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin: 4px 0 8px;
    padding: 9px 11px;
    border-radius: 10px;
    background: var(--bubble);
    font-size: 12px;
    line-height: 1.4;
  }

  .fact {
    color: var(--text);
  }

  .how {
    color: var(--text-dim);
  }
</style>
