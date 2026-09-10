<script lang="ts">
  /**
   * The top right corner of an open dialogue: how much of the five hour and
   * of the seven day window is gone, and of the model window when the plan
   * has one.
   *
   * The context ring stood here between v60 and v65 and has gone back to the
   * row under the field, where the original keeps it and where the thing it
   * acts on is being written. These stay: they are about the account, not
   * about this conversation. tech.md 6.4, 6.12 and 6.15.
   */
  import UsageDial from './UsageDial.svelte';

  let {
    hour,
    week,
    scoped = null,
  }: {
    hour: number | null;
    week: number | null;
    /**
     * The week a plan counts for one model on its own, named as the server
     * named it. Absent for a plan that counts nothing apart, and then there
     * is no third dial at all: a dash would promise a number that is never
     * coming. tech.md 6.4.
     */
    scoped?: { label: string; pct: number | null } | null;
  } = $props();
</script>

<div class="corner">
  <UsageDial pct={hour} label="5h" size={13} />
  <UsageDial pct={week} label="7d" size={13} />
  {#if scoped}
    <UsageDial pct={scoped.pct} label={scoped.label} size={13} />
  {/if}
</div>

<style>
  .corner {
    display: flex;
    align-items: center;
    gap: 10px;
    /* Never wider than its half of the band beside the notch: the dials give
       up their gaps first, and a name too long for what is left is cut rather
       than drawn under the cutout, where it cannot be seen. tech.md 6.7. */
    min-width: 0;
    overflow: hidden;
  }
</style>
