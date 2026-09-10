<script lang="ts">
  /**
   * The top right corner of an open dialogue: how much of the five hour and
   * of the seven day window is gone.
   *
   * The context ring stood here between v60 and v65 and has gone back to the
   * row under the field, where the original keeps it and where the thing it
   * acts on is being written. These two stay: they are about the account, not
   * about this conversation. tech.md 6.12 and 6.15.
   */
  import UsageDial from './UsageDial.svelte';

  let {
    hour,
    week,
  }: {
    hour: number | null;
    week: number | null;
  } = $props();
</script>

<div class="corner">
  <UsageDial pct={hour} label="5h" size={13} />
  <UsageDial pct={week} label="7d" size={13} />
</div>

<style>
  .corner {
    display: flex;
    align-items: center;
    gap: 10px;
    /* Never wider than its half of the band beside the notch: nothing is
       drawn where the cutout is, because there are no pixels there to draw
       on. tech.md 6.7. */
    min-width: 0;
    overflow: hidden;
  }
</style>
