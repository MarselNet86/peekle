<script lang="ts">
  import { commands } from '$lib/bridge';
  import { createFeed } from '$lib/features/feed/feed.svelte';
  import { createIsland } from '$lib/features/island/island.svelte';
  import { choiceFor, isPermission } from '$lib/features/permission/permission.svelte';
  import {
    hideSession,
    openList,
    openSession,
    renameSession,
    sessionOf,
  } from '$lib/features/sessions/sessions.svelte';
  import { createUsage } from '$lib/features/usage/usage.svelte';
  import type { Delivery } from '$lib/types/generated/Delivery';
  import { scrollState } from '$lib/logic/feed';
  import { searchSessions } from '$lib/logic/sessions';
  import { clickPutsAway, restStatus } from '$lib/logic/rest';
  import Button from '$lib/ui/Button.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import PermissionRow from '$lib/ui/PermissionRow.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import RestMark from '$lib/ui/RestMark.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import SearchField from '$lib/ui/SearchField.svelte';
  import TypingLine from '$lib/ui/TypingLine.svelte';
  import SessionRow from '$lib/ui/SessionRow.svelte';
  import UsageBar from '$lib/ui/UsageBar.svelte';
  import UsageDial from '$lib/ui/UsageDial.svelte';
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
  const rows = $derived(current?.entries ?? []);
  const listing = $derived(island.view === 'Sessions');
  let query = $state('');
  const cards = $derived(searchSessions(feed.sessions, query));

  // The feed scrolls for real, so where it stands is a fact about the DOM
  // rather than about the number of rows. tech.md 6.12.
  let scroller = $state<HTMLElement | null>(null);
  let atBottom = $state(true);
  let showHint = $state(false);

  function readScroll() {
    if (!scroller) return;
    const state = scrollState(scroller.scrollTop, scroller.clientHeight, scroller.scrollHeight);
    atBottom = state.atBottom;
    showHint = state.showHint;
  }

  function toBottom(smooth = true) {
    if (!scroller) return;
    scroller.scrollTo({ top: scroller.scrollHeight, behavior: smooth ? 'smooth' : 'auto' });
  }

  // Opening a session lands on the last message: a messenger that opens on the
  // first one reads as broken. Standing at the bottom is a fact about one feed,
  // so switching feeds forgets it; carrying it over opens the next session
  // wherever the last one happened to be. tech.md 6.12.
  let shown = $state('');

  $effect(() => {
    const id = JSON.stringify(island.view);
    const switched = id !== shown;
    shown = id;

    void rows.length;
    void cards.length;
    if (!scroller) return;

    const follow = switched || atBottom;
    // Twice: once for the rows, once after the spring has finished growing the
    // shape around them. A single frame lands halfway up a still opening feed.
    requestAnimationFrame(() => {
      if (follow) toBottom(false);
      readScroll();
    });
    const settle = setTimeout(() => {
      if (follow) toBottom(false);
      readScroll();
    }, 260);
    return () => clearTimeout(settle);
  });
  // The mark is all the user sees while the island rests, so it carries the
  // one bit worth acting on. tech.md 6.7.
  const resting = $derived(restStatus(feed.sessions, island.prompt !== null));

  // A request that is still waiting is the reason to come back, so the mark
  // opens its session rather than the list. tech.md 6.7.
  function reopen() {
    const waiting = island.prompt?.session.session_id;
    if (waiting) openSession(waiting);
    else openList();
  }
  // The ring on the mark and the 5h bar inside read the same number, so they
  // come from the same place. tech.md 6.7.
  const hourWindow = $derived(usage.bars[0]?.pct ?? null);

  // Where the text would go if it were typed right now. Asked of Rust on every
  // session change, because a pane can close between two keystrokes and a
  // cached answer goes stale exactly when it matters. tech.md 6.5.
  let delivery = $state<Delivery | null>(null);

  $effect(() => {
    const id = sessionOf(island.view);
    if (!id) {
      delivery = null;
      return;
    }
    // Re-asked whenever the feed moves: a session that just ended stops being
    // reachable, and the field has to say so without waiting for a click.
    void current?.status;
    void current?.entries.length;

    let live = true;
    commands.deliveryFor(id).then((next) => {
      if (live) delivery = next;
    });
    return () => {
      live = false;
    };
  });

  const reachable = $derived(delivery !== null && delivery !== 'Unreachable');
  const steering = $derived(current !== undefined && island.driving === current.session.session_id);

  // The field says which channel it has, because the two differ in when the
  // text lands, and a promise that hides the difference is a promise the
  // product cannot keep. tech.md 6.5.
  const replyHint = $derived.by(() => {
    if (island.prompt) return 'Reply to Claude';
    if (delivery === null) return 'Send to Claude';
    if (delivery === 'Unreachable') return 'This session has ended';
    // Held and Resume both leave at once; TurnBoundary waits for a turn that
    // Peekle is not holding, and saying so is the whole point of the hint.
    if (delivery === 'TurnBoundary') return 'Type now, it goes when Claude stops';
    return 'Message Claude';
  });
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

  // Queued text has left the field the moment it is in the feed.
  function send(text: string) {
    if (!current) return;
    island.answer(text, current.session.session_id);
    reply = '';
  }

  $effect(() => {
    const stop = Promise.all([island.start(), feed.start(), usage.start()]);
    // Rust holds the panel back until this lands, so the island never appears
    // as an empty shape. tech.md section 8.
    commands.windowReady('island');
    return () => {
      void stop.then((offs) => offs.forEach((off) => off()));
    };
  });

  // An open island takes the mouse on the whole 720 by 560 window, so a click
  // beside the shape is already lost to whatever is underneath. Spending it on
  // putting the island away is the one useful thing left to do with it.
  //
  // A pending request does not stop it: collapsing is not resolving, the hook
  // stays pending, and the mark keeps pulsing until it is answered. tech.md 6.7.
  $effect(() => {
    if (island.view === 'Collapsed') return;

    const dismiss = (event: MouseEvent) => {
      if (clickPutsAway(island.view, event.target)) commands.setView('Collapsed');
    };

    window.addEventListener('click', dismiss);
    return () => window.removeEventListener('click', dismiss);
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

<!-- The windows are the same fact in both views: how much of the hour is gone
     is as much a part of reading a dialogue as of picking one. tech.md S7. -->
{#snippet usage_bars()}
  <div class="usage">
    {#if usage.connectLabel}
      <Button
        label={usage.connecting ? 'Connecting' : usage.connectLabel}
        variant="connect"
        disabled={usage.connecting}
        wide
        onclick={() => usage.connect()}
      />
    {:else}
      {#each usage.bars as bar (bar.label)}
        <UsageBar label={bar.label} pct={bar.pct} resetsAt={bar.resetsAt} reason={usage.reason} />
      {/each}
    {/if}
  </div>
{/snippet}

<div class="island" bind:this={host}>
  <Shape view={island.view} notch={island.notch}>
    {#snippet rest()}
      <RestMark status={resting} pct={hourWindow} onopen={() => reopen()} />
    {/snippet}

    {#if island.view === 'Pill' && island.toast}
      <Toast text={island.toast.text} tone={island.toast.tone} badge={island.toast.badge} />
    {:else if listing}
      <div class="feed">
        <!-- Only once there is a list worth searching. tech.md S14. -->
        {#if feed.sessions.length > 3}
          <div class="search"><SearchField bind:value={query} /></div>
        {/if}
        <div class="rows" bind:this={scroller} onscroll={readScroll}>
          {#each cards as card (card.session.session_id)}
            <SessionRow
              {card}
              onopen={() => openSession(card.session.session_id)}
              onrename={(title) => renameSession(card.session.session_id, title)}
              onhide={() => hideSession(card.session.session_id)}
            />
          {/each}
          <!-- An empty list opened from the mark says so. Collapsing on the
               click the user just made reads as a broken island. tech.md S12. -->
          {#if cards.length === 0}
            <p class="empty">
              {query
                ? 'Nothing matches that.'
                : 'No sessions yet. Start Claude Code and it shows up here.'}
            </p>
          {/if}
        </div>
        <ScrollHint visible={showHint} onclick={() => toBottom()} />

        {@render usage_bars()}
      </div>
    {:else if current}
      <div class="feed">
        <div class="head">
          <button class="back" onclick={() => openList()} aria-label="Back to the session list">
            <svg viewBox="0 0 8 12" width="8" height="12" aria-hidden="true">
              <path d="M6.5 1l-5 5 5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
            </svg>
            <span>{current.session.project}</span>
          </button>
          <!-- Holding a turn blocks the whole session, so it is a switch the
               user throws rather than something Peekle decides. tech.md 6.5. -->
          <button
            class="steer"
            class:on={steering}
            onclick={() => island.takeover(current.session.session_id, !steering)}
            title={steering
              ? 'Hand this session back to Claude Code (⌥⇧S)'
              : 'Steer this session from the island (⌥⇧S)'}
          >
            {steering ? 'Steering' : 'Take over'}
          </button>
          <!-- The windows in miniature. The full bars stay in the list, where
               there is room for them. tech.md 6.12. -->
          <div class="dials">
            <UsageDial pct={usage.bars[0]?.pct ?? null} label="5h" size={13} />
            <UsageDial pct={usage.bars[1]?.pct ?? null} label="7d" size={13} />
          </div>
        </div>
        <div class="rows" bind:this={scroller} onscroll={readScroll}>
          {#each rows as entry (entry.id)}
            <FeedRow {entry} />
          {/each}
          <!-- The agent is mid turn, so the dialogue says so instead of sitting
               still and reading as broken. tech.md 6.12. -->
          {#if current.status === 'Working'}
            <TypingLine />
          {/if}
        </div>
        <ScrollHint visible={showHint} onclick={() => toBottom()} />
        {#if usage.connectLabel}
          {@render usage_bars()}
        {/if}

        {#if permission}
          <div class="reply">
            <PermissionRow
              request={permission}
              onallow={() => answerPermission('allow')}
              ondeny={() => answerPermission('deny')}
            />
          </div>
        {:else}
          <!-- Always here, and dark only when there is genuinely nowhere to
               deliver. Hiding it made the island look broken; a field that
               lies about delivery would be worse. tech.md 6.5. -->
          <div class="reply">
            <PromptInput
              bind:value={reply}
              placeholder={replyHint}
              disabled={!reachable}
              onsubmit={send}
              onescape={() => island.dismiss()}
            />
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

  /* The feed scrolls natively. Every row is in the markup: a window of six
     was the reason the chevron did nothing. tech.md 6.12. */
  .rows {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: none;
  }

  /* The bars sit under the session list, where the eye lands after reading
     what is running. They never gate the island opening: it draws on the last
     snapshot and a fresh one arrives as an event. tech.md 6.4. */
  .usage {
    flex: none;
    border-top: 1px solid var(--hairline);
    padding-top: 6px;
  }

  .search {
    flex: none;
    padding-bottom: 6px;
  }

  .empty {
    margin: 0;
    padding: 10px 2px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex: none;
  }

  .dials {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 4px;
  }

  /* Off it reads as an offer, on it reads as a state, because on it is
     costing the user their extension. tech.md 6.5. */
  .steer {
    margin-left: auto;
    flex: none;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 10px;
    padding: 2px 8px;
    margin-bottom: 4px;
    cursor: pointer;
  }

  .steer:hover {
    color: var(--text);
  }

  .steer.on {
    border-color: transparent;
    background: var(--brand);
    color: #000;
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
</style>
