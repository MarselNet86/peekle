<script lang="ts">
  import { commands } from '$lib/bridge';
  import { createFeed } from '$lib/features/feed/feed.svelte';
  import { createIsland } from '$lib/features/island/island.svelte';
  import { choiceFor, isPermission } from '$lib/features/permission/permission.svelte';
  import { openList, openSession, sessionOf } from '$lib/features/sessions/sessions.svelte';
  import { createUsage } from '$lib/features/usage/usage.svelte';
  import { feedWindow } from '$lib/logic/feed';
  import { restStatus } from '$lib/logic/rest';
  import Button from '$lib/ui/Button.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import PermissionRow from '$lib/ui/PermissionRow.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import RestMark from '$lib/ui/RestMark.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import SessionRow from '$lib/ui/SessionRow.svelte';
  import UsageBar from '$lib/ui/UsageBar.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  // Rust measures the notch and hands both dimensions over in the query
  // string, because a borderless webview reports no safe area of its own.
  const island = createIsland(typeof location === 'undefined' ? '' : location.search);
  const feed = createFeed();
  const usage = createUsage();

  let host = $state<HTMLElement | null>(null);

  const current = $derived.by(() => {
    const id = sessionOf(island.view);
    return id ? feed.card(id) : undefined;
  });
  const rows = $derived(feedWindow(current?.entries ?? []));
  const listing = $derived(island.view === 'Sessions');
  const cards = $derived(feedWindow(feed.sessions));
  // The mark is all the user sees while the island rests, so it carries the
  // one bit worth acting on. tech.md 6.7.
  const resting = $derived(restStatus(feed.sessions));
  // The ring on the mark and the 5h bar inside read the same number, so they
  // come from the same place. tech.md 6.7.
  const hourWindow = $derived(usage.bars[0]?.pct ?? null);

  // The field is live only while a session is actually waiting on an answer.
  // Outside that there is nowhere to deliver the text, and a field that looks
  // ready but goes nowhere is worse than one that is plainly off. tech.md 6.5.
  const waiting = $derived(current?.status === 'WaitingOnUser' && island.prompt !== null);
  const permission = $derived(isPermission(island.prompt) ? island.prompt : null);

  function answerPermission(kind: 'allow' | 'deny') {
    const choice = choiceFor(island.prompt, kind);
    if (choice) island.choose(choice);
  }

  let reply = $state('');

  // A settled request leaves nothing behind for the next one to inherit.
  $effect(() => {
    if (!island.prompt) reply = '';
  });

  $effect(() => {
    const stop = Promise.all([island.start(), feed.start(), usage.start()]);
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
    {#snippet rest()}
      <RestMark status={resting} pct={hourWindow} onopen={() => openList()} />
    {/snippet}

    {#if island.view === 'Pill' && island.toast}
      <Toast text={island.toast.text} tone={island.toast.tone} badge={island.toast.badge} />
    {:else if listing}
      <div class="feed">
        <div class="rows">
          {#each cards.visible as card (card.session.session_id)}
            <SessionRow {card} onopen={() => openSession(card.session.session_id)} />
          {/each}
          <!-- An empty list opened from the mark says so. Collapsing on the
               click the user just made reads as a broken island. tech.md S12. -->
          {#if cards.visible.length === 0}
            <p class="empty">No sessions yet. Start Claude Code and it shows up here.</p>
          {/if}
        </div>
        <ScrollHint visible={cards.showScrollHint} />

        <div class="usage">
          {#each usage.bars as bar (bar.label)}
            <UsageBar
              label={bar.label}
              pct={bar.pct}
              resetsAt={bar.resetsAt}
              reason={usage.reason}
            />
          {/each}
        </div>
      </div>
    {:else if current}
      <div class="feed">
        <button class="back" onclick={() => openList()} aria-label="Back to the session list">
          <svg viewBox="0 0 8 12" width="8" height="12" aria-hidden="true">
            <path d="M6.5 1l-5 5 5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
          </svg>
          <span>{current.session.project}</span>
        </button>
        <div class="rows">
          {#each rows.visible as entry (entry.id)}
            <FeedRow {entry} />
          {/each}
        </div>
        <ScrollHint visible={rows.showScrollHint && !waiting} />

        {#if permission}
          <div class="reply">
            <PermissionRow
              request={permission}
              onallow={() => answerPermission('allow')}
              ondeny={() => answerPermission('deny')}
            />
          </div>
        {:else if waiting}
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

  /* The bars sit under the session list, where the eye lands after reading
     what is running. They never gate the island opening: it draws on the last
     snapshot and a fresh one arrives as an event. tech.md 6.4. */
  .usage {
    flex: none;
    border-top: 1px solid var(--hairline);
    padding-top: 6px;
  }

  .empty {
    margin: 0;
    padding: 10px 2px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .back {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    border: none;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 11px;
    padding: 0 0 4px;
    cursor: pointer;
  }

  .back:hover {
    color: var(--text);
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
