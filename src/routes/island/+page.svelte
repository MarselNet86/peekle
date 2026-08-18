<script lang="ts">
  import { commands } from '$lib/bridge';
  import { createFeed } from '$lib/features/feed/feed.svelte';
  import { createIsland } from '$lib/features/island/island.svelte';
  import { feedWindow } from '$lib/logic/feed';
  import Button from '$lib/ui/Button.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  // Rust measures the notch and hands both dimensions over in the query
  // string, because a borderless webview reports no safe area of its own.
  const island = createIsland(typeof location === 'undefined' ? '' : location.search);
  const feed = createFeed();

  let host = $state<HTMLElement | null>(null);

  const openSession = $derived(
    typeof island.view === 'object' ? feed.card(island.view.Session) : undefined,
  );
  const rows = $derived(feedWindow(openSession?.entries ?? []));

  // The field is live only while a session is actually waiting on an answer.
  // Outside that there is nowhere to deliver the text, and a field that looks
  // ready but goes nowhere is worse than one that is plainly off. tech.md 6.5.
  const waiting = $derived(openSession?.status === 'WaitingOnUser' && island.prompt !== null);

  let reply = $state('');

  // A settled request leaves nothing behind for the next one to inherit.
  $effect(() => {
    if (!island.prompt) reply = '';
  });

  $effect(() => {
    const stop = Promise.all([island.start(), feed.start()]);
    // Rust holds the panel back until this lands, so the island never appears
    // as an empty shape. tech.md section 8.
    commands.windowReady('island');
    return () => {
      void stop.then((offs) => offs.forEach((off) => off()));
    };
  });

  // Rust records what the shape actually measured and changes nothing with it:
  // the window never resizes. Sampled until the spring stops moving, because a
  // size read mid flight describes a frame that no longer exists. tech.md 6.7.
  $effect(() => {
    const box = host?.querySelector('.shape');
    if (!(box instanceof HTMLElement)) return;

    // Re-run on every view change: that is what starts the spring.
    void island.view;

    let frame = 0;
    let last = { width: -1, height: -1 };
    let still = 0;

    const sample = () => {
      const size = { width: box.offsetWidth, height: box.offsetHeight };
      still = size.width === last.width && size.height === last.height ? still + 1 : 0;
      last = size;

      // Two identical frames mean the spring has come to rest.
      if (still >= 2) {
        commands.islandBounds(size.width, size.height);
        return;
      }
      frame = requestAnimationFrame(sample);
    };
    frame = requestAnimationFrame(sample);

    return () => cancelAnimationFrame(frame);
  });
</script>

<div class="island" bind:this={host}>
  <Shape view={island.view} notch={island.notch}>
    {#if island.view === 'Pill' && island.toast}
      <Toast text={island.toast.text} tone={island.toast.tone} badge={island.toast.badge} />
    {:else if openSession}
      <div class="feed">
        <div class="rows">
          {#each rows.visible as entry (entry.id)}
            <FeedRow {entry} />
          {/each}
        </div>
        <ScrollHint visible={rows.showScrollHint && !waiting} />

        {#if waiting}
          <div class="reply">
            <PromptInput
              bind:value={reply}
              placeholder="Reply to Claude"
              onsubmit={(text) => island.answer(text)}
              onescape={() => island.dismiss()}
            />
            <div class="choices">
              {#each island.prompt?.options ?? [] as option (option.id)}
                <Button
                  label={option.label}
                  variant={option.kind === 'Continue' ? 'primary' : 'ghost'}
                  onclick={() => island.choose(option.id)}
                />
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </Shape>
</div>

<style>
  .island {
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .feed {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 6px 14px 8px;
    box-sizing: border-box;
  }

  .rows {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .reply {
    flex: none;
    border-top: 1px solid var(--hairline);
    padding-top: 6px;
  }

  .choices {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    padding-top: 6px;
  }
</style>
