<script lang="ts">
  import { untrack } from 'svelte';

  import { commands, fileSrc } from '$lib/bridge';
  import { createAgent } from '$lib/features/agent/agent.svelte';
  import { createNotify, NOTIFY_HINT } from '$lib/features/notify/notify.svelte';
  import { createBadge, BADGE_HINT } from '$lib/features/usage/badge.svelte';
  import { createFeed } from '$lib/features/feed/feed.svelte';
  import { createIsland } from '$lib/features/island/island.svelte';
  import { choiceFor, isPermission, isQuestion } from '$lib/features/permission/permission.svelte';
  import {
    deleteSession,
    openList,
    openSession,
    renameSession,
    sessionOf,
  } from '$lib/features/sessions/sessions.svelte';
  import { createShots } from '$lib/features/shots/shots.svelte';
  import { createUsage } from '$lib/features/usage/usage.svelte';
  import { createSignIn } from '$lib/features/signin/signin.svelte';
  import {
    contextLabel,
    noteTitle,
    settingsNote,
    MODE_NOTE,
    type SettingsNote,
  } from '$lib/logic/agent';
  import { scrollAim, scrollState } from '$lib/logic/feed';
  import { attachable, fileName, SPLIT_PATH_NOTE } from '$lib/logic/files';
  import { canPickFolder, CHOOSE, folderOptions } from '$lib/logic/folders';
  import { feedRows } from '$lib/logic/work';
  import {
    barred as isBarred,
    canContinue as canContinueCard,
    classifyContinueOutcome,
    FORKED_NOTE,
    replyReachable,
    stopAvailable,
    searchSessions,
    steadyOrder,
    ASKED_TO_STOP,
    STOP_ASKED_NOTE,
  } from '$lib/logic/sessions';
  import { isShot, shotName } from '$lib/logic/shots';
  import { clickSettles, restStatus } from '$lib/logic/rest';
  import AgentBar from '$lib/ui/AgentBar.svelte';
  import Button from '$lib/ui/Button.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import PickerMenu from '$lib/ui/PickerMenu.svelte';
  import NoteBlock from '$lib/ui/NoteBlock.svelte';
  import SignInPanel from '$lib/ui/SignInPanel.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import PermissionRow from '$lib/ui/PermissionRow.svelte';
  import QuestionPrompt from '$lib/ui/QuestionPrompt.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import AskPanel from '$lib/ui/AskPanel.svelte';
  import RestMark from '$lib/ui/RestMark.svelte';
  import SearchField from '$lib/ui/SearchField.svelte';
  import Toggle from '$lib/ui/Toggle.svelte';
  import UsageCorner from '$lib/ui/UsageCorner.svelte';
  import WorkLine from '$lib/ui/WorkLine.svelte';
  import SessionRow from '$lib/ui/SessionRow.svelte';
  import ShotChip from '$lib/ui/ShotChip.svelte';
  import ShotPreview from '$lib/ui/ShotPreview.svelte';
  import ShotPrompt from '$lib/ui/ShotPrompt.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import Sign from '$lib/ui/Sign.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  // Rust measures the notch and hands both dimensions over in the query
  // string, because a borderless webview reports no safe area of its own.
  const island = createIsland(typeof location === 'undefined' ? '' : location.search);
  const feed = createFeed();
  const usage = createUsage();
  const signIn = createSignIn();
  const shots = createShots();
  const agent = createAgent();
  const notify = createNotify();
  const badge = createBadge();

  let host = $state<HTMLElement | null>(null);
  // Whether the gear has the list open on settings instead. One shape, so the
  // setting stands where the list stood rather than in a window of its own.
  // tech.md 6.17.
  let settingsOpen = $state(false);

  /** Which session is open, by id. A string rather than the card: the card is
   * a new object on every hook, and while a turn runs those arrive on every
   * tool call, so anything watching it for "the session changed" fires
   * constantly. tech.md 6.12. */
  const openId = $derived(sessionOf(island.view));
  const current = $derived.by(() => (openId ? feed.card(openId) : undefined));
  /** A question is standing in this dialogue and nothing else may move.
   * tech.md 6.14. */
  const asking = $derived(isQuestion(island.prompt));
  // Everything said, in order, with each run of calls folded into one line
  // that carries a clock. tech.md 6.12.
  // `working` is false while a question stands, and that is not a lie about
  // the session: the agent is parked on the question and doing nothing at
  // all. A clock ticking above a question says the opposite of what is true,
  // and a spinner beside one somebody is reading is movement asking for
  // attention it has no business taking. What it worked through before asking
  // stays, as the finished line it is. tech.md 6.14.
  // The compact this dialogue is in the middle of, or null. tech.md 6.21.
  const compacting = $derived(current?.compacting ?? null);
  /** When the island asked this chat to stop and the turn has not ended yet.
   * tech.md 6.5. */
  const askedToStop = $derived(current?.stopping ?? null);
  /** The CLI is asking whether this folder is trusted, and until that is
   * answered it runs nothing at all. tech.md 6.24. */
  const askingTrust = $derived(current?.asking_trust != null);
  let answeringTrust = $state(false);

  async function answerTrust(trust: boolean) {
    if (!current || answeringTrust) return;
    answeringTrust = true;
    try {
      await commands.answerTrust(current.session.session_id, trust);
    } catch (err) {
      startError = String(err);
    } finally {
      answeringTrust = false;
    }
  }
  const rows = $derived(
    feedRows(
      current?.entries ?? [],
      // A compact takes the working line off the feed for the same reason a
      // question does: the agent is not working, the CLI is, and two lines
      // about one pause are two answers to one question. tech.md 6.21.
      current?.status === 'Working' && !asking && compacting === null,
    ),
  );
  // The word the work line says while a stop request stands. One word rather
  // than the cycle: what is happening is known, and inventing `Reading` over
  // it says less than the truth does. tech.md 6.5.
  const words = $derived(askedToStop === null ? undefined : [ASKED_TO_STOP]);
  // A view naming a session the feed does not have falls back to the list.
  // The alternative is what it used to do: render none of the branches and
  // leave an empty black shape on screen, which reads as a crash.
  const listing = $derived(
    island.view === 'Sessions' || (sessionOf(island.view) !== undefined && current === undefined),
  );
  let query = $state('');
  // The order the list opened with. Rows keep their places while it is on
  // screen, so a turn in a chat nobody is watching cannot move the row under
  // the cursor. Taken again on the next opening. tech.md 6.12.
  let held = $state<string[]>([]);
  $effect(() => {
    if (!listing) {
      held = [];
      return;
    }
    if (held.length === 0) {
      held = feed.sessions.map((card) => card.session.session_id);
    }
  });
  const cards = $derived(searchSessions(steadyOrder(feed.sessions, held), query));

  // The feed scrolls for real, so where it stands is a fact about the DOM
  // rather than about the number of rows. tech.md 6.12.
  let scroller = $state<HTMLElement | null>(null);
  let atBottom = $state(true);

  function readScroll() {
    if (!scroller) return;
    // Only whether the reader is at the end, which decides whether a new row
    // may follow the feed down. Nothing is drawn about it any more: the hint
    // was a chevron over the last line of the conversation, and the scrollbar
    // already says there is more. tech.md 6.12.
    atBottom = scrollState(
      scroller.scrollTop,
      scroller.clientHeight,
      scroller.scrollHeight,
    ).atBottom;
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

  // Every reading is judged, and one in ten of them is news: the window
  // stepping into a new ten is the only thing the resting island announces
  // about usage. tech.md 6.18.
  $effect(() => {
    badge.track(hourWindow);
  });

  // A preview the switch promised plays when the island is back on the bezel,
  // which is the only place the badge exists. tech.md 6.18.
  $effect(() => {
    if (island.view === 'Collapsed') badge.rest();
  });

  // Whether this session has an input field at all. One question with one
  // answer: the island types into sessions it started, and into nothing else.
  // A session whose process has exited is no longer one of them. tech.md 6.5.
  const owned = $derived(
    current !== undefined && current.origin === 'Owned' && current.status !== 'Ended',
  );
  // An observed chat is answerable too: the first reply forks it into one we
  // own, which is what Desktop does when you type into an old chat. No button
  // and no ceremony -- a chat is a chat. tech.md 6.5.
  const canContinue = $derived(canContinueCard(current));
  // The fork is a real round trip -- spawning a process, not a fire-and-forget
  // write to a pty already open -- so the field has to say it is busy and stop
  // taking presses while it is, or a person who sees no change from their
  // first press sends the same reply again into a second race to fork the
  // same chat. tech.md 6.5.
  let continuing = $state(false);
  const reachable = $derived(
    replyReachable({ hasPrompt: island.prompt !== null, owned, canContinue, continuing }),
  );

  // Whether there is a turn to stop, and whether a press is already on its
  // way. One press at a time: Esc twice into a pty is still one interrupt,
  // but two requests into an inbox are two turns. The control is the field's
  // own button, which becomes the way to stop while a turn runs. tech.md 6.5.
  const canStop = $derived(
    stopAvailable({
      status: current?.status,
      hasPrompt: island.prompt !== null,
      owned,
      canContinue,
      asked: askedToStop,
    }),
  );
  // Why the row only reads, said out loud because it was pressed. A tooltip
  // answers a question the reader already has; a press is the question being
  // asked, and it deserves an answer where the eye already is. It goes on the
  // next look elsewhere: it is an answer, not a state. tech.md 6.15.
  let rowNote = $state<SettingsNote | null>(null);
  $effect(() => {
    // The id, and never the card. Reading the card here subscribed this to
    // every hook that touched the session, and while a turn runs those come
    // with every tool call: the answer to a press was wiped a second after
    // it was asked for, which is exactly when it is being read. It is spent
    // by its own term, by the cross, or by leaving for another session.
    // tech.md 6.15.
    void openId;
    rowNote = null;
  });

  let stopping = $state(false);
  async function stop() {
    if (!current || stopping) return;
    stopping = true;
    try {
      await commands.stopSession(current.session.session_id);
    } catch (err) {
      startError = String(err);
    } finally {
      stopping = false;
    }
  }

  // Two words, and neither is an excuse: every chat takes text since v66, so
  // the field never has a reason to say it cannot. tech.md 6.5.
  const replyHint = $derived.by(() => {
    if (island.prompt) return 'Reply to Claude';
    if (continuing) return 'Sending…';
    return 'Message Claude';
  });
  // Starting a session is what makes one talkable-to, so the island needs a
  // way to do it. The folder comes from a project the island already knows,
  // because there is no folder picker in the product and inventing a path is
  // worse than reusing one. tech.md 6.5.
  const newestCwd = $derived(feed.sessions[0]?.session.cwd ?? null);
  let startError = $state<string | null>(null);

  // The folder of a chat that has not begun. It is the project name in the
  // head band until the first message, and a name that is still being chosen
  // is drawn as the choice it is. tech.md 6.23.
  const aiming = $derived(canPickFolder(current));
  const folders = $derived(folderOptions(feed.sessions, current?.session.cwd ?? ''));

  async function pickFolder(id: string) {
    if (!current) return;
    const sessionId = current.session.session_id;
    // The last row is not a folder: it is the way to the rest of the disk.
    const cwd = id === CHOOSE ? await commands.chooseFolder() : id;
    // A cancel changes nothing and says nothing: the person changed their
    // mind, and that is not an event. tech.md 6.23.
    if (!cwd) return;
    try {
      await commands.setSessionCwd(sessionId, cwd);
    } catch (err) {
      startError = String(err);
    }
  }

  async function startSession(cwd: string) {
    startError = null;
    // No agent is spawned behind a screen that says the account is not
    // reachable. The button is not offered while barred either; this is the
    // second lock on the same door. tech.md 6.16.
    if (barred) return;
    try {
      const session = await commands.startSession(cwd);
      if (session) openSession(session.session_id);
    } catch (err) {
      startError = String(err);
    }
  }

  // One attempt at getting the words into a chat the island does not hold the
  // process of: into the inbox of the live process that holds it, into a
  // resume of our own when nothing does, or into a copy when it is held and
  // takes nothing. Which of the three is Rust's call at the instant of
  // sending, and the id that comes back is where the words actually landed.
  // tech.md 6.5.
  async function attemptContinue(sessionId: string, text: string, paths: string[]) {
    try {
      const session = await commands.continueSession(sessionId, text, paths);
      return classifyContinueOutcome(session, undefined);
    } catch (err) {
      return classifyContinueOutcome(undefined, err);
    }
  }

  // What answers in the session on screen, and which of the three controls is
  // still waiting on the agent to confirm a pick. tech.md 6.15.
  const setup = $derived(current?.agent ?? null);
  const waiting = $derived(agent.pendingFor(current?.session.session_id ?? '', setup));
  // What was chosen and has not landed yet. The row stands on it while it
  // travels: a pick that leaves the old value on screen reads as a pick that
  // did nothing, which is exactly how it read. tech.md 6.15.
  const asked = $derived(agent.asked(current?.session.session_id ?? ''));

  // A chat the user opened has nothing to show until the account is reachable,
  // so the way in stands where the chat would be rather than under it. Never
  // while a hook is waiting: answering a live permission request is the one
  // thing the island exists for, and it must work whatever usage says.
  // tech.md 6.4 and 6.16.
  const barred = $derived(
    isBarred({ outOfReach: usage.outOfReach, hasPrompt: island.prompt !== null }),
  );

  const permission = $derived(isPermission(island.prompt) ? island.prompt : null);
  const question = $derived(isQuestion(island.prompt) ? island.prompt : null);

  function answerPermission(kind: 'allow' | 'deny') {
    const choice = choiceFor(island.prompt, kind);
    if (choice) island.choose(choice);
  }

  // The draft of each chat, by id. One field for all chats was the rule until
  // v80.16, and a reply begun in one chat stood in the field of the next one
  // opened -- by hand or by another chat's turn ending. tech.md 6.7.
  let drafts = $state<Record<string, string>>({});
  const reply = $derived(openId ? (drafts[openId] ?? '') : '');
  function setReply(text: string) {
    if (openId) drafts[openId] = text;
  }
  /**
   * The plus at the start of the row: files from disk, attached by path.
   *
   * Nothing is copied anywhere -- the file is already on disk and Claude Code
   * opens it itself once it is named -- so the path goes straight into the
   * row above the field and travels with the message. tech.md 6.25.
   */
  /** Why the last deletion did not happen, and whose row said so. Cleared by
   * the next attempt: one fault at a time, on the row it belongs to.
   * tech.md 6.26. */
  let deleteFault = $state<{ id: string; why: string } | null>(null);

  async function remove(sessionId: string) {
    deleteFault = null;
    const why = await deleteSession(sessionId);
    if (why) deleteFault = { id: sessionId, why };
  }

  async function attachFiles() {
    if (!current) return;
    const id = current.session.session_id;
    let picked: string[] | null;
    try {
      picked = await commands.chooseFiles();
    } catch (err) {
      startError = String(err);
      return;
    }
    // A cancel is not an event and says nothing. Neither is a route rendered
    // with no Tauri under it, which answers null. tech.md 6.25.
    if (!picked || picked.length === 0) return;
    for (const path of picked.filter(attachable)) shots.attach(id, path);
    startError = picked.every(attachable) ? null : SPLIT_PATH_NOTE;
  }

  // What is waiting in the field of the session on screen. tech.md 6.13.
  const attached = $derived(current ? shots.of(current.session.session_id) : []);
  // A screenshot has a picture to show and open; a file has a name and no
  // more. One list, two kinds, told apart by the shape of the path.
  // tech.md 6.13 and 6.25.
  const chips = $derived(attached.map((path) => ({ path, shot: isShot(path) })));
  // Whether the island is being written in: the cursor in the field with the
  // panel key, or something unsent in it. Rust stops the leave clock and keeps
  // pills and other chats' turns off it while this holds. The cursor alone is
  // not enough: the field focuses itself on mount, panel key or not, and a
  // page nobody clicked into is not being written in. tech.md 6.7.
  let fieldFocused = $state(false);
  let pageFocused = $state(false);
  const composing = $derived(
    current !== undefined &&
      ((fieldFocused && pageFocused) || reply.trim().length > 0 || attached.length > 0),
  );
  $effect(() => {
    commands.setComposing(composing);
  });
  // Asked rather than only listened for: the panel can stop being the key
  // window while the field stays first responder -- the screenshot frame, an
  // app coming to the front -- and WebKit does not promise a blur for that.
  // A quarter second is far under the 800ms leave clock. tech.md 6.7.
  $effect(() => {
    if (!current) return;
    const read = () => (pageFocused = document.hasFocus());
    read();
    const tick = setInterval(read, 250);
    window.addEventListener('focus', read);
    window.addEventListener('blur', read);
    return () => {
      clearInterval(tick);
      window.removeEventListener('focus', read);
      window.removeEventListener('blur', read);
    };
  });
  // Which attachment is open at full size. A layer over the content, so it is
  // the route's and not Rust's: no view changes and the window keeps its size.
  let opened = $state<string | null>(null);
  // Whether what is open was opened from the row above the field. Those go
  // when the attachment goes; a picture opened out of the feed was sent long
  // ago and nothing above the field owns it. Without this the two were one
  // rule, and a shot pressed in a message closed itself in the same tick it
  // opened: it is in no reply being written, so the rule below took it.
  // tech.md 6.13.
  let openedFromField = $state(false);

  function openShot(path: string, fromField: boolean) {
    opened = path;
    openedFromField = fromField;
  }

  // An attachment taken back, or a message sent, takes its picture with it.
  // So does a collapse from any other cause: a picture left open would come
  // back up over whatever the island opens on next.
  $effect(() => {
    if (opened === null) return;
    if (island.view === 'Collapsed' || (openedFromField && !attached.includes(opened))) {
      opened = null;
    }
  });

  // Rust puts an island away when the pointer has been off it for 800ms, and
  // reaching for the click that closes the picture takes the pointer off it
  // first. A picture the user opened by hand outranks that. tech.md 6.13.
  $effect(() => {
    commands.setPreview(opened !== null);
  });

  // A settled request leaves nothing behind for the next one to inherit.
  // Untracked, because the draft is the open chat's: read through the
  // dependency, this cleared the draft of every chat on the way in.
  $effect(() => {
    if (!island.prompt) untrack(() => setReply(''));
  });

  // Queued text has left the field the moment it is in the feed.
  //
  // A permission request takes the text instead, and takes it alone: there is
  // no attachment on that path, so the shots stay in the field for the message
  // that comes after it. tech.md 6.13.
  async function send(text: string) {
    if (!current) return;
    const answering = island.prompt !== null;

    // A chat we hold the process of takes the words down its own pty. Every
    // other chat -- finished, started elsewhere, held by another app -- goes
    // the continue route, and which of its three ways the words take is
    // Rust's call at the moment of sending. Invisible on purpose: Desktop has
    // no button for this either, a chat is just a chat. The text is only
    // cleared once it has somewhere to go. tech.md 6.5.
    const id = current.session.session_id;
    if (!answering && !owned) {
      startError = null;
      // The field goes busy for the one request that is an actual round
      // trip: PromptInput stops taking presses the instant this flips, which
      // is what a second Enter before the first fork lands used to race.
      // tech.md 6.5.
      continuing = true;
      const outcome = await attemptContinue(id, text, attached);
      continuing = false;

      if (outcome.ok) {
        shots.clear(id);
        setReply('');
        // A different id means the chat was held elsewhere and this is a copy
        // of it. Said out loud where the conversation now is: an id that
        // changes silently reads as the island losing the chat. tech.md 6.5.
        if (outcome.sessionId !== id) rowNote = FORKED_NOTE;
        openSession(outcome.sessionId);
      } else if (outcome.error) {
        startError = outcome.error;
      }
      return;
    }

    island.answer(text, id, attached);
    if (!answering) shots.clear(id);
    setReply('');
  }

  $effect(() => {
    const stop = Promise.all([
      island.start(),
      feed.start(),
      usage.start(),
      signIn.start(),
      shots.start(),
      agent.start(),
      notify.start().then(() => () => {}),
      badge.start(),
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

    // Where the press began. A drag that starts on the shape and ends beside
    // it is a selection, and its click lands on the document. tech.md 6.7.
    let pressed: EventTarget | null = null;
    const press = (event: PointerEvent) => {
      pressed = event.target;
    };
    const dismiss = (event: MouseEvent) => {
      const began = pressed;
      pressed = null;
      // An open picture is what a click beside the shape is aimed at, so it
      // takes it: collapsing would carry off the feed and the reply with it.
      // tech.md 6.13.
      switch (clickSettles(island.view, event.target, opened !== null, began)) {
        case 'preview':
          opened = null;
          break;
        case 'island':
          commands.setView('Collapsed');
          break;
      }
    };

    window.addEventListener('pointerdown', press, true);
    window.addEventListener('click', dismiss);
    return () => {
      window.removeEventListener('pointerdown', press, true);
      window.removeEventListener('click', dismiss);
    };
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
      // The whole rectangle, offset included: a shape floating off the edge
      // (6.7) is where it is drawn, not where a rectangle pinned to the top
      // would put it, and Rust builds nothing of its own. tech.md 6.5.
      const rect = box.getBoundingClientRect();
      const size = { width: rect.width, height: rect.height };
      still = size.width === last.width && size.height === last.height ? still + 1 : 0;
      last = size;

      // Two identical frames mean the spring has come to rest.
      if (still >= 2) {
        frame = 0;
        commands.islandBounds({
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
        });
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
  {#if usage.outOfReach}
    <!-- The account screen, in the strip under the list. Says what is wrong
         and offers the one thing that fixes it. tech.md 6.16. -->
    <div class="usage">{@render accountPanel(true)}</div>
  {:else if usage.connectLabel || usage.failed}
    <div class="usage">
      {#if usage.connectLabel}
        <Button
          label={usage.connecting ? 'Connecting' : usage.connectLabel}
          variant="connect"
          busy={usage.connecting}
          wide
          onclick={() => usage.connect()}
        />
      {/if}
      <!-- Why it is not connected, in words. A press that changes nothing on
           screen reads as a press that was lost. tech.md 6.4. -->
      {#if usage.failed && !usage.connecting}
        <p class="why">{usage.reason}</p>
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet accountPanel(compact = false)}
  <SignInPanel
    {compact}
    signIn={signIn.state}
    reason={usage.reasonCode}
    busy={signIn.busy || usage.connecting}
    onaction={() => (usage.needsSignIn ? signIn.begin() : usage.connect())}
    oncode={(code) => signIn.submit(code)}
    onopen={() => signIn.openPage()}
    oncancel={() => signIn.cancel()}
  />
{/snippet}

<div class="island" bind:this={host}>
  <!-- A question outgrows the dialogue, so the shape takes the window while
       one stands. tech.md 6.14. -->
  <Shape
    view={island.view}
    notch={island.notch}
    badge={badge.wide}
    {asking}
    deep={island.toast?.detail != null}
  >
    {#snippet rest()}
      <RestMark status={resting} pct={hourWindow} badge={badge.value} onopen={() => reopen()} />
    {/snippet}

    <!-- A screenshot is waiting to be attached, and it outranks a toast: the
         offer runs out in seconds and a toast can be read afterwards. 6.13. -->
    <!-- A permission asks for yes or no, and neither answer needs the feed.
         The panel carries the question; pressing it anywhere but the buttons
         lands in the session it came from. tech.md 6.7. -->
    {#if island.view === 'Ask' && permission}
      <AskPanel
        request={permission}
        onallow={() => answerPermission('allow')}
        ondeny={() => answerPermission('deny')}
        onopen={() => openSession(permission.session.session_id)}
      />
    {:else if island.view === 'Pill' && shots.offer}
      <ShotPrompt
        project={shots.offer.project}
        left={shots.left}
        secs={shots.secs}
        onopen={() => shots.offer && openSession(shots.offer.session_id)}
      />
    {:else if island.view === 'Pill' && island.toast}
      <!-- Keyed on the notice itself, so a second one restarts the hairline
           under it rather than inheriting however much the first had left.
           Every notice arrives as its own object. tech.md 6.2. -->
      {#key island.toast}
        <Toast
          text={island.toast.text}
          detail={island.toast.detail}
          tookMs={island.toast.took_ms}
          tone={island.toast.tone}
          badge={island.toast.badge}
          ttlMs={island.toast.ttl_ms}
          onopen={island.toast.session ? () => openSession(island.toast!.session!) : null}
        />
      {/key}
    {:else if listing}
      <div class="feed">
        {#if usage.gateSessions}
          <!-- Nothing else is drawn: no search, no rows, no New session.
               History a person has never granted access to is not history
               they can browse yet, and a half-working list beside a button
               that may or may not do anything is worse than one clear ask.
               tech.md 6.4. -->
          <div class="gate">
            {#if usage.outOfReach}
              {@render accountPanel()}
            {:else}
              <Button
                label={usage.connecting ? 'Connecting' : (usage.connectLabel ?? 'Connect')}
                variant="connect"
                busy={usage.connecting}
                wide
                onclick={() => usage.connect()}
              />
              {#if usage.failed && !usage.connecting}
                <p class="why">{usage.reason}</p>
              {/if}
            {/if}
          </div>
        {:else}
          <!-- The gear stands in the corner and nowhere else: the list is
               what this view is for, and a settings row above every session
               would be a permanent tax on the thing people came to read.
               tech.md 6.17. -->
          <div class="top" class:open={settingsOpen}>
            <!-- The way out stands where a way out stands, on the left and
                 pointing back, and it appears with the thing it leaves. A
                 view that can be entered and not left is a trap, however
                 small the view. -->
            {#if settingsOpen}
              <IconButton
                name="back"
                title="Back to the session list"
                onclick={() => (settingsOpen = false)}
              />
            {/if}
            <!-- The bug and the gear travel together so the way back keeps
                 the left end to itself. The gear stays in the very corner:
                 it stood there first, and a button that arrived later does
                 not take its place. tech.md 6.22. -->
            <div class="corner">
              <IconButton
                name="bug"
                title="Report a bug"
                hint="Tell the developer what broke. Opens Telegram."
                onclick={() => commands.openBugReport()}
              />
              <IconButton
                name="settings"
                title="Settings"
                pressed={settingsOpen}
                onclick={() => (settingsOpen = !settingsOpen)}
              />
            </div>
          </div>
          {#if settingsOpen}
            <div class="settings">
              <Toggle
                label="Notify when a turn ends"
                hint={NOTIFY_HINT}
                checked={notify.on}
                busy={notify.busy}
                onchange={(next) => notify.set(next)}
              />
              {#if notify.error}
                <p class="empty">{notify.error}</p>
              {/if}
              <!-- Turning it on shows the badge at once: the next ten may be an
                   hour away, and a switch nobody can see work is a switch
                   nobody believes. tech.md 6.18. -->
              <Toggle
                label="Show usage on the island"
                hint={BADGE_HINT}
                checked={badge.on}
                busy={badge.busy}
                onchange={(next) => badge.set(next, hourWindow)}
              />
            </div>
          {:else}
            <!-- Only once there is a list worth searching. tech.md S14. -->
            {#if feed.sessions.length > 3}
              <div class="search"><SearchField bind:value={query} /></div>
            {/if}
            <!-- Not offered while the account is out of reach: a new chat needs
               an agent, and starting one behind a screen that says the island
               cannot reach the account is a session nobody can use. The way in
               stands at the bottom of this list instead. tech.md 6.16. -->
            {#if newestCwd && !barred}
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
                  ondelete={() => remove(card.session.session_id)}
                  fault={deleteFault?.id === card.session.session_id ? deleteFault.why : null}
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

            {@render connect()}
          {/if}
        {/if}
      </div>
    {:else if barred}
      <!-- Instead of the chat, not over it: a conversation the island cannot
           reach is a field that takes words and delivers none. The block says
           what is wrong; a header repeating it would be the second title on a
           screen that has room for one. tech.md 6.16. -->
      <div class="feed">
        <div class="head">
          <button class="back" onclick={() => openList()} aria-label="Back to the session list">
            <svg viewBox="0 0 8 12" width="8" height="12" aria-hidden="true">
              <path
                d="M6.5 1L1.5 6l5 5"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
        </div>
        <div class="blocked">{@render accountPanel()}</div>
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
            <span class="project">{current.session.project}</span>
          </button>
          <!-- Everything that says how much is left, in one corner: the two
               windows and the context. The ring is the button that compacts.
               tech.md 6.12 and 6.15. -->
          <UsageCorner hour={usage.bars[0]?.pct ?? null} week={usage.bars[1]?.pct ?? null} />
        </div>
        <!-- The answer to a press stands here, above the feed, where the eye
             lands when something does not happen. Under the input it sat
             below what the reader was looking at. tech.md 6.15 and 9. -->
        {#if rowNote}
          <div class="note">
            <NoteBlock fact={rowNote.fact} how={rowNote.how} onclose={() => (rowNote = null)} />
          </div>
        {/if}
        <div class="rows" bind:this={scroller} onscroll={readScroll}>
          <!-- A chat that has nothing in it yet carries the sign instead of
               half a window of black. Nothing has been said and nothing is
               going: `feedRows` puts a work line in from the first second of
               a turn (6.12), so an empty list of rows means exactly that.
               tech.md 6.12 and 9. -->
          {#if rows.length === 0 && compacting === null}
            <div class="blank"><Sign caption="Let's begin" /></div>
          {/if}
          {#each rows as row (row.id)}
            {#if row.kind === 'said'}
              <FeedRow
                entry={row.entry}
                shotSrc={fileSrc}
                onopenshot={(path) => openShot(path, false)}
              />
            {:else}
              <!-- A whole run of calls, as the one thing asked of it: whether
                   the agent is out, and for how long. tech.md 6.12. -->
              <WorkLine running={row.to === null} from={row.from} to={row.to} {words} />
            {/if}
          {/each}
          <!-- The one line about the one pause the CLI takes on its own: it
               runs for minutes, the agent answers nothing through it, and a
               feed that says nothing reads as a feed that died. tech.md 6.21. -->
          {#if compacting}
            <WorkLine running tone="compact" words={['Compacting']} from={compacting.since} />
          {/if}
        </div>
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
          <!-- The shape has already grown to the window for this (6.14). What
               a very long question still cannot fit scrolls here, because an
               option cut off by the bottom edge is an option that is not
               there. -->
          <div class="reply asking">
            <QuestionPrompt
              questions={question.questions}
              onsubmit={(answers) => island.answerQuestions(answers)}
              onclose={() => island.dismiss()}
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
                {#each chips as chip (chip.path)}
                  <ShotChip
                    name={chip.shot ? shotName(chip.path) : fileName(chip.path)}
                    kind={chip.shot ? 'shot' : 'file'}
                    src={chip.shot ? fileSrc(chip.path) : ''}
                    onopen={chip.shot ? () => openShot(chip.path, true) : undefined}
                    onremove={() => current && shots.remove(current.session.session_id, chip.path)}
                  />
                {/each}
              </div>
            {/if}
            {#if agent.error}
              <p class="empty">{agent.error}</p>
            {/if}
            {#if startError}
              <p class="empty">{startError}</p>
            {/if}
            <!-- Where the agent will work, above the field and at its left
                 edge: the folder is part of what is about to be said, and it
                 is chosen in the same breath as the first message. It stands
                 only while it can be taken -- nothing said in this chat yet.
                 tech.md 6.23. -->
            <!-- The CLI asks whether this folder is trusted before it runs
                 anything, and it asks on its own screen, where nobody can see
                 it: no hook fires and no transcript is written until it is
                 answered. So the question stands here instead, and the person
                 answers it. Peekle never answers it for them -- it is about
                 whether they vouch for what is in the folder. tech.md 6.24. -->
            {#if askingTrust}
              <div class="trust">
                <p class="asked">Claude Code asks whether you trust this folder</p>
                <p class="where">{current.session.cwd}</p>
                <div class="answers">
                  <Button
                    label="Trust it"
                    variant="prominent"
                    busy={answeringTrust}
                    onclick={() => answerTrust(true)}
                  />
                  <Button
                    label="Not here"
                    variant="muted"
                    disabled={answeringTrust}
                    onclick={() => answerTrust(false)}
                  />
                </div>
              </div>
            {/if}
            {#if aiming}
              <div class="aim">
                <PickerMenu
                  label={current.session.project}
                  icon="folder"
                  options={folders}
                  value={current.session.cwd}
                  onpick={(id) => pickFolder(id)}
                />
              </div>
            {/if}
            <PromptInput
              bind:value={() => reply, setReply}
              onfocuschange={(focused) => (fieldFocused = focused)}
              placeholder={replyHint}
              disabled={!reachable}
              working={(canStop || askedToStop !== null) && !stopping}
              onsubmit={send}
              onstop={() => (askedToStop === null ? stop() : (rowNote = STOP_ASKED_NOTE))}
              onescape={() => island.dismiss()}
              onpasteimage={() => current && shots.paste(current.session.session_id)}
              onattach={attachFiles}
            >
              <!-- In the capsule, under the text: what will answer what is
                   being typed belongs to the typing. tech.md 6.15. -->
              {#snippet tools()}
                <AgentBar
                  agent={setup}
                  defaults={agent.defaults}
                  models={agent.models}
                  live={owned}
                  note={noteTitle(settingsNote(current))}
                  contextTitle={contextLabel(setup, owned) || 'Nothing in the context yet'}
                  askedModel={asked.model}
                  askedEffort={asked.effort}
                  askedMode={asked.mode}
                  mode={current.mode}
                  canPickMode={owned}
                  thinking={current.thinking}
                  canSetThinking={owned && setup === null}
                  ultra={agent.ultra(current.session.session_id)}
                  canUltra={owned && setup !== null}
                  pendingModel={waiting.model}
                  pendingEffort={waiting.effort}
                  pendingMode={agent.modePending(current.session.session_id, current.mode)}
                  pendingCompact={waiting.compact}
                  onmodel={(alias) => current && agent.setModel(current.session.session_id, alias)}
                  oneffort={(level) =>
                    current && agent.setEffort(current.session.session_id, level)}
                  onultra={() => current && agent.setUltracode(current.session.session_id)}
                  oncompact={() => current && agent.compact(current.session.session_id, setup)}
                  onmode={(next) => current && agent.setMode(current.session.session_id, next)}
                  onmodenote={() => (rowNote = MODE_NOTE)}
                  onthinking={(on) => current && agent.setThinking(current.session.session_id, on)}
                  onnote={() => (rowNote = settingsNote(current))}
                />
              {/snippet}
            </PromptInput>
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
    /* No top padding: the row that stood there has gone up into the band
       beside the cutout (6.7), and what is left starts at the shape's own
       edge. The bottom is 14 rather than 8 -- the field and the last row of
       the list both stood on the kerb. tech.md 9. */
    padding: 0 14px 14px;
    box-sizing: border-box;
  }

  /* The band beside the notch, taken by whichever row stands at the top of a
     view: the head of a dialogue, the gear of the list. Lifted by exactly the
     height of the cutout, so it costs the content below nothing, and held to
     that height so it reads as one row with the menu bar beside it. The gap
     is the cutout itself: nothing is drawn across it, because across it there
     are no pixels to draw on. Zero on a display with no notch, and then this
     is an ordinary row at the top. tech.md 6.7. */
  .head,
  .top {
    margin-top: calc(-1 * var(--notch-h, 0px));
    min-height: var(--notch-h, 0px);
  }

  /* Neither end of the row reaches past its own side of the cutout, however
     long a project is named: what crosses the hole cannot be seen at all, so
     it is cut here instead. Half the row, less half the cutout, less the
     breathing room the eye needs beside a hole. tech.md 6.7. */
  .head > :global(*) {
    max-width: calc(50% - var(--notch-w, 0px) / 2 - 6px);
  }

  /* A layer over the conversation, not a row in it. Standing in the flow, it
     pushed every message down on arrival and pulled them back up on leaving,
     so reading jumped twice for something that is not part of the reading.
     It sits under the head, inset with the rows, and the feed below is
     untouched. tech.md 9. */
  .note {
    position: absolute;
    top: 4px;
    left: 14px;
    right: 14px;
    z-index: 3;
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

  /* The sign, in the room an empty dialogue has nothing else to put in it.
     It takes the whole scroller so it stands in the middle of the feed rather
     than at the top of it, and sits a little above centre -- optical centre,
     because the composer below weighs the bottom of the shape down. */
  .blank {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100%;
    padding-bottom: 22px;
    box-sizing: border-box;
  }

  /* The bars sit under the session list, where the eye lands after reading
     what is running. They never gate the island opening: it draws on the last
     snapshot and a fresh one arrives as an event. tech.md 6.4. */
  .usage {
    flex: none;
    border-top: 1px solid var(--hairline);
    padding-top: 10px;
  }

  /* Fills the list's whole space rather than sitting among rows: nothing
     else is drawn beside it, so the button is the one thing on screen.
     tech.md 6.4. */
  .gate {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    gap: 10px;
  }

  /* A chat the island cannot reach. The block sits a little above centre --
     optical centre, not arithmetic: a shape whose top carries a chevron reads
     as bottom-heavy when its content is measured from the edges. */
  .blocked {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 18px 42px;
  }

  /* Under the button, dim and small: it explains, it does not shout. */
  .why {
    margin: 6px 2px 0;
    color: var(--text-dim);
    font-size: 11px;
    text-align: center;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex: none;
  }

  .corner {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  /* With a way out on the left, the two controls take the ends of the row
     between them. */
  .top.open {
    justify-content: space-between;
  }

  .settings {
    flex: none;
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
    /* Wide enough to clear the cutout between the two ends of the row. */
    gap: calc(var(--notch-w, 0px) + 10px);
    flex: none;
  }

  .start {
    padding: 0 2px 8px;
  }

  /* Off it reads as an offer, on it reads as a state, because on it is
     costing the user their extension. tech.md 6.5. */
  .back {
    display: flex;
    align-items: center;
    gap: 6px;
    /* It shrinks and the corner does not: a project name can be any length,
       and the two windows are always the same two words. */
    flex: 0 1 auto;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 11px;
    padding: 0;
    cursor: pointer;
  }

  /* Cut rather than drawn under the cutout, where there is nothing to draw
     on. tech.md 6.7. */
  .project {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .back:hover {
    color: var(--text);
  }

  /* No rule above the field. The capsule draws its own edge now (6.15), and
     a hairline over it is a second border for one boundary -- two lines where
     the eye reads one thing. Space separates them instead. tech.md 6.12. */
  .reply {
    flex: none;
    padding-top: 10px;
  }

  /* Shrinks before it overflows, and scrolls what is left over. The feed
     above gives up its room first: it has `min-height: 0` and nothing in it
     is being asked a question. tech.md 6.14. */
  .reply.asking {
    flex: 0 1 auto;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
    /* The answer rows bleed ten pixels either side of the text to make room
       for their own hover ground, so the scroller has to be that much wider
       than the column: a scroller narrower than what it holds scrolls
       sideways too, and the first thing to go is the digit at the head of
       every row. tech.md 9. */
    margin: 0 -10px;
    padding-left: 10px;
    padding-right: 10px;
  }

  .attached {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 2px 8px;
  }

  /* Its own line over the capsule, held to the same left edge the field has:
     a control tucked into the row under the text would be a fourth thing in a
     row that is already about the answer, not about where it happens. */
  .aim {
    display: flex;
    padding: 0 4px 5px;
  }

  /* Over the field, where the answer is given, and with its own ground: it is
     a question about this chat, not a line in it. tech.md 6.24. */
  .trust {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0 2px 8px;
    padding: 10px 12px;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    background: var(--surface);
  }

  .asked {
    margin: 0;
    color: var(--text);
    font-size: 12px;
  }

  .where {
    margin: 0;
    color: var(--text-dim);
    font-size: 11px;
    overflow-wrap: anywhere;
  }

  .answers {
    display: flex;
    gap: 8px;
  }
</style>
