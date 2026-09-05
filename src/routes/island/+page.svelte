<script lang="ts">
  import { commands, fileSrc } from '$lib/bridge';
  import { createAgent } from '$lib/features/agent/agent.svelte';
  import { createFeed } from '$lib/features/feed/feed.svelte';
  import { createIsland } from '$lib/features/island/island.svelte';
  import { choiceFor, isPermission, isQuestion } from '$lib/features/permission/permission.svelte';
  import {
    hideSession,
    openList,
    openSession,
    renameSession,
    sessionOf,
  } from '$lib/features/sessions/sessions.svelte';
  import { createShots } from '$lib/features/shots/shots.svelte';
  import { createUsage } from '$lib/features/usage/usage.svelte';
  import { scrollAim, scrollState } from '$lib/logic/feed';
  import { canContinue as canContinueCard, searchSessions } from '$lib/logic/sessions';
  import { shotName } from '$lib/logic/shots';
  import { clickSettles, restStatus } from '$lib/logic/rest';
  import AgentBar from '$lib/ui/AgentBar.svelte';
  import Button from '$lib/ui/Button.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import PermissionRow from '$lib/ui/PermissionRow.svelte';
  import QuestionPrompt from '$lib/ui/QuestionPrompt.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import RestMark from '$lib/ui/RestMark.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import SearchField from '$lib/ui/SearchField.svelte';
  import TypingLine from '$lib/ui/TypingLine.svelte';
  import SessionRow from '$lib/ui/SessionRow.svelte';
  import ShotChip from '$lib/ui/ShotChip.svelte';
  import ShotPreview from '$lib/ui/ShotPreview.svelte';
  import ShotPrompt from '$lib/ui/ShotPrompt.svelte';
  import UsageDial from '$lib/ui/UsageDial.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  // Rust measures the notch and hands both dimensions over in the query
  // string, because a borderless webview reports no safe area of its own.
  const island = createIsland(typeof location === 'undefined' ? '' : location.search);
  const feed = createFeed();
  const usage = createUsage();
  const shots = createShots();
  const agent = createAgent();

  let host = $state<HTMLElement | null>(null);

  const current = $derived.by(() => {
    const id = sessionOf(island.view);
    return id ? feed.card(id) : undefined;
  });
  const rows = $derived(current?.entries ?? []);
  // A view naming a session the feed does not have falls back to the list.
  // The alternative is what it used to do: render none of the branches and
  // leave an empty black shape on screen, which reads as a crash.
  const listing = $derived(
    island.view === 'Sessions' || (sessionOf(island.view) !== undefined && current === undefined),
  );
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

  function toTop() {
    scroller?.scrollTo({ top: 0, behavior: 'auto' });
  }

  /** The list opens on its freshest row, a dialogue on its last message. Both
   * live in the same scroller, so the view decides. tech.md 6.12. */
  function land(aim: ReturnType<typeof scrollAim>) {
    if (aim === 'bottom') toBottom(false);
    if (aim === 'top') toTop();
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

    const aim = scrollAim(listing ? 'list' : 'feed', switched, atBottom);
    // Twice: once for the rows, once after the spring has finished growing the
    // shape around them. A single frame lands halfway up a still opening feed.
    requestAnimationFrame(() => {
      land(aim);
      readScroll();
    });
    const settle = setTimeout(() => {
      land(aim);
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

  // Whether this session has an input field at all. One question with one
  // answer: the island types into sessions it started, and into nothing else.
  // A session whose process has exited is no longer one of them. tech.md 6.5.
  const owned = $derived(
    current !== undefined && current.origin === 'Owned' && current.status !== 'Ended',
  );
  const reachable = $derived(island.prompt !== null || owned);

  // An observed chat that nobody owns and no live client is writing can be
  // forked into an owned one, the way Desktop opens an existing chat. There is
  // no live-client signal in the webview, so Rust makes the final call and
  // refuses if it is being written; here it is offered whenever the session is
  // not already ours. tech.md 6.5.
  const canContinue = $derived(canContinueCard(current));

  const replyHint = $derived.by(() => {
    if (island.prompt) return 'Reply to Claude';
    if (owned) return 'Message Claude';
    if (current?.status === 'Ended') return 'This session has finished';
    // No lie about delivery and no dead field without a reason: Peekle cannot
    // type into a process it did not start, and says which one it is.
    return 'Started outside Peekle, so this one is read-only';
  });
  // Starting a session is what makes one talkable-to, so the island needs a
  // way to do it. The folder comes from a project the island already knows,
  // because there is no folder picker in the product and inventing a path is
  // worse than reusing one. tech.md 6.5.
  const newestCwd = $derived(feed.sessions[0]?.session.cwd ?? null);
  let startError = $state<string | null>(null);

  async function startSession(cwd: string) {
    startError = null;
    try {
      const session = await commands.startSession(cwd);
      if (session) openSession(session.session_id);
    } catch (err) {
      startError = String(err);
    }
  }

  // Fork this observed chat into one we own and open it. The original card
  // stays in the list as history. tech.md 6.5.
  async function continueSession(sessionId: string) {
    startError = null;
    try {
      const session = await commands.continueSession(sessionId);
      if (session) openSession(session.session_id);
    } catch (err) {
      startError = String(err);
    }
  }

  // What answers in the session on screen, and which of the three controls is
  // still waiting on the agent to confirm a pick. tech.md 6.15.
  const setup = $derived(current?.agent ?? null);
  const waiting = $derived(agent.pendingFor(current?.session.session_id ?? '', setup));

  const permission = $derived(isPermission(island.prompt) ? island.prompt : null);
  const question = $derived(isQuestion(island.prompt) ? island.prompt : null);

  function answerPermission(kind: 'allow' | 'deny') {
    const choice = choiceFor(island.prompt, kind);
    if (choice) island.choose(choice);
  }

  let reply = $state('');
  // What is waiting in the field of the session on screen. tech.md 6.13.
  const attached = $derived(current ? shots.of(current.session.session_id) : []);
  // Which attachment is open at full size. A layer over the content, so it is
  // the route's and not Rust's: no view changes and the window keeps its size.
  let opened = $state<string | null>(null);

  // An attachment taken back, or a message sent, takes its picture with it.
  // So does a collapse from any other cause: a picture left open would come
  // back up over whatever the island opens on next.
  $effect(() => {
    if (opened === null) return;
    if (!attached.includes(opened) || island.view === 'Collapsed') opened = null;
  });

  // Rust puts an island away when the pointer has been off it for 800ms, and
  // reaching for the click that closes the picture takes the pointer off it
  // first. A picture the user opened by hand outranks that. tech.md 6.13.
  $effect(() => {
    commands.setPreview(opened !== null);
  });

  // A settled request leaves nothing behind for the next one to inherit.
  $effect(() => {
    if (!island.prompt) reply = '';
  });

  // Queued text has left the field the moment it is in the feed.
  //
  // A permission request takes the text instead, and takes it alone: there is
  // no attachment on that path, so the shots stay in the field for the message
  // that comes after it. tech.md 6.13.
  function send(text: string) {
    if (!current) return;
    const id = current.session.session_id;
    const answering = island.prompt !== null;

    island.answer(text, id, attached);
    if (!answering) shots.clear(id);
    reply = '';
  }

  $effect(() => {
    const stop = Promise.all([
      island.start(),
      feed.start(),
      usage.start(),
      shots.start(),
      agent.start(),
    ]);
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
      // An open picture is what a click beside the shape is aimed at, so it
      // takes it: collapsing would carry off the feed and the reply with it.
      // tech.md 6.13.
      switch (clickSettles(island.view, event.target, opened !== null)) {
        case 'preview':
          opened = null;
          break;
        case 'island':
          commands.setView('Collapsed');
          break;
      }
    };

    window.addEventListener('click', dismiss);
    return () => window.removeEventListener('click', dismiss);
  });

  // Rust records what the shape actually measured and changes nothing with it:
  // the window never resizes. Sampled until the spring stops moving, because a
  // size read mid flight describes a frame that no longer exists. tech.md 6.7.
  //
  // Watched rather than tied to the view: content moves the shape too. A row
  // of attachments appearing after a screenshot makes the island a row taller
  // at the same view, and a rectangle measured before it leaves the bottom
  // strip -- the field and the chips in it -- outside the island as far as the
  // pointer tracker is concerned, which then puts the island away under a
  // pointer that never left it.
  $effect(() => {
    const box = host?.querySelector('.shape');
    if (!(box instanceof HTMLElement)) return;

    let frame = 0;
    let last = { width: -1, height: -1 };
    let still = 0;

    const sample = () => {
      const size = { width: box.offsetWidth, height: box.offsetHeight };
      still = size.width === last.width && size.height === last.height ? still + 1 : 0;
      last = size;

      // Two identical frames mean the spring has come to rest.
      if (still >= 2) {
        frame = 0;
        commands.islandBounds(size.width, size.height);
        return;
      }
      frame = requestAnimationFrame(sample);
    };

    // Idle until something moves, so a resting island costs no frames.
    const measure = () => {
      if (frame) return;
      still = 0;
      frame = requestAnimationFrame(sample);
    };

    const watch = new ResizeObserver(measure);
    watch.observe(box);
    measure();

    return () => {
      watch.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  });
</script>

<!-- Only the offer to connect. The numbers themselves live as dials in the
     dialogue header and as the ring on the resting mark, and a third copy of
     them under the session list was costing the list the rows it is for.
     tech.md S7. -->
{#snippet connect()}
  {#if usage.connectLabel}
    <div class="usage">
      <Button
        label={usage.connecting ? 'Connecting' : usage.connectLabel}
        variant="connect"
        disabled={usage.connecting}
        wide
        onclick={() => usage.connect()}
      />
    </div>
  {/if}
{/snippet}

<div class="island" bind:this={host}>
  <Shape view={island.view} notch={island.notch}>
    {#snippet rest()}
      <RestMark status={resting} pct={hourWindow} onopen={() => reopen()} />
    {/snippet}

    <!-- A screenshot is waiting to be attached, and it outranks a toast: the
         offer runs out in seconds and a toast can be read afterwards. 6.13. -->
    {#if island.view === 'Pill' && shots.offer}
      <ShotPrompt project={shots.offer.project} left={shots.left} secs={shots.secs} />
    {:else if island.view === 'Pill' && island.toast}
      <Toast text={island.toast.text} tone={island.toast.tone} badge={island.toast.badge} />
    {:else if listing}
      <div class="feed">
        <!-- Only once there is a list worth searching. tech.md S14. -->
        {#if feed.sessions.length > 3}
          <div class="search"><SearchField bind:value={query} /></div>
        {/if}
        {#if newestCwd}
          <div class="start">
            <Button label="New session" onclick={() => startSession(newestCwd)} wide />
          </div>
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
                : 'No sessions yet. Run Claude Code once in a project and Peekle picks it up.'}
            </p>
          {/if}
          {#if startError}
            <p class="empty">{startError}</p>
          {/if}
        </div>
        <ScrollHint visible={showHint} onclick={() => toBottom()} />

        {@render connect()}
      </div>
    {:else if current}
      <div class="feed">
        <!-- Over the dialogue rather than instead of it: the reply being
             written stays where it was. tech.md 6.13. -->
        {#if opened}
          <ShotPreview
            name={shotName(opened)}
            src={fileSrc(opened)}
            onclose={() => (opened = null)}
          />
        {/if}
        <div class="head">
          <button class="back" onclick={() => openList()} aria-label="Back to the session list">
            <svg viewBox="0 0 8 12" width="8" height="12" aria-hidden="true">
              <path d="M6.5 1l-5 5 5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
            </svg>
            <span>{current.session.project}</span>
          </button>
          <!-- The gesture the open dialogue invites: another go at this same
               project, in a session the island owns. -->
          <button
            class="new"
            onclick={() => startSession(current.session.cwd)}
            title="Start a new session in this project"
            aria-label="Start a new session in this project"
          >
            +
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
        {@render connect()}

        {#if permission}
          <div class="reply">
            <PermissionRow
              request={permission}
              onallow={() => answerPermission('allow')}
              ondeny={() => answerPermission('deny')}
            />
          </div>
        {:else if question}
          <div class="reply">
            <QuestionPrompt
              questions={question.questions}
              onsubmit={(answers) => island.answerQuestions(answers)}
            />
          </div>
        {:else}
          <!-- Always here, and dark only when there is genuinely nowhere to
               deliver. Hiding it made the island look broken; a field that
               lies about delivery would be worse. tech.md 6.5. -->
          <div class="reply">
            <!-- Attached, not sent: the shot waits here until the user says
                 what they want done with it. tech.md 6.13. -->
            {#if attached.length > 0}
              <div class="attached">
                {#each attached as path (path)}
                  <ShotChip
                    name={shotName(path)}
                    src={fileSrc(path)}
                    onopen={() => (opened = path)}
                    onremove={() => current && shots.remove(current.session.session_id, path)}
                  />
                {/each}
              </div>
            {/if}
            <!-- On the divider, above the field: a session is aimed before it
                 is spoken to, not after. tech.md 6.15. -->
            <AgentBar
              agent={setup}
              defaults={agent.defaults}
              models={agent.models}
              live={owned}
              pendingModel={waiting.model}
              pendingEffort={waiting.effort}
              pendingCompact={waiting.compact}
              onmodel={(alias) => current && agent.setModel(current.session.session_id, alias)}
              oneffort={(level) => current && agent.setEffort(current.session.session_id, level)}
              oncompact={() => current && agent.compact(current.session.session_id, setup)}
            />
            {#if agent.error}
              <p class="empty">{agent.error}</p>
            {/if}
            {#if startError}
              <p class="empty">{startError}</p>
            {/if}
            <!-- An observed chat has no field, but it can be forked into one we
                 own, the way Desktop opens an existing chat. Rust refuses if a
                 live client is writing it. tech.md 6.5. -->
            {#if canContinue}
              <Button
                label="Continue this chat"
                onclick={() => current && continueSession(current.session.session_id)}
                wide
              />
            {:else}
              <PromptInput
                bind:value={reply}
                placeholder={replyHint}
                disabled={!reachable}
                onsubmit={send}
                onescape={() => island.dismiss()}
              />
            {/if}
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
    position: relative;
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

  .start {
    padding: 0 2px 8px;
  }

  .new {
    margin-left: auto;
    flex: none;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 13px;
    line-height: 1;
    width: 20px;
    height: 20px;
    margin-bottom: 4px;
    cursor: pointer;
  }

  .new:hover {
    color: var(--text);
  }

  .dials {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 4px;
  }

  /* Off it reads as an offer, on it reads as a state, because on it is
     costing the user their extension. tech.md 6.5. */
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
    padding-top: 10px;
  }

  .attached {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 2px 8px;
  }
</style>
