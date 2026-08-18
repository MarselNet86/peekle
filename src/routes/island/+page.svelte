<script lang="ts">
  import { commands } from '$lib/bridge';
  import { createIsland } from '$lib/features/island/island.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  // Rust measures the notch and hands both dimensions over in the query
  // string, because a borderless webview reports no safe area of its own.
  const island = createIsland(typeof location === 'undefined' ? '' : location.search);

  let host = $state<HTMLElement | null>(null);

  $effect(() => {
    const stop = island.start();
    // Rust holds the panel back until this lands, so the island never appears
    // as an empty shape. tech.md section 8.
    commands.windowReady('island');
    return () => {
      void stop.then((off) => off());
    };
  });

  /** The spring of 6.10 is at rest well inside this. */
  const SETTLE_MS = 420;

  // Rust records what the shape actually measured and changes nothing with it:
  // the window never resizes. Read once the spring has stopped, because a size
  // sampled mid flight describes a frame that no longer exists. tech.md 6.7.
  //
  // getBoundingClientRect, not offsetWidth: this webview keeps handing back the
  // pre-animation integer from offsetWidth long after the box has moved.
  $effect(() => {
    // Read the view first. Registering the dependency only after the element
    // check would leave the effect asleep for good if the first run is early.
    void island.view;

    const box = host?.querySelector('.shape');
    if (!(box instanceof HTMLElement)) return;

    const timer = setTimeout(() => {
      const rect = box.getBoundingClientRect();
      commands.islandBounds(rect.width, rect.height);
    }, SETTLE_MS);

    return () => clearTimeout(timer);
  });
</script>

<div class="island" bind:this={host}>
  <Shape view={island.view} notch={island.notch}>
    {#if island.toast}
      <Toast text={island.toast.text} tone={island.toast.tone} badge={island.toast.badge} />
    {/if}
  </Shape>
</div>

<style>
  .island {
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }
</style>
