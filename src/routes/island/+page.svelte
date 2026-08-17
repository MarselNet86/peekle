<script lang="ts">
  import { commands } from '$lib/bridge';
  import { createIsland } from '$lib/features/island/island.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  const island = createIsland();

  $effect(() => {
    const stop = island.start();
    // Rust holds the panel back until this lands, so a toast never appears as
    // an empty pill. tech.md section 8.
    commands.windowReady('island');
    return () => {
      void stop.then((off) => off());
    };
  });
</script>

<div class="island">
  {#if island.toast}
    {#key island.toast.text}
      <Toast text={island.toast.text} tone={island.toast.tone} badge={island.toast.badge} />
    {/key}
  {/if}
</div>

<style>
  .island {
    width: 100vw;
    height: 100vh;
  }
</style>
