<script lang="ts">
  let notifyOn = $state(true);
  import Kbd from '$lib/ui/Kbd.svelte';
  import LabelPill from '$lib/ui/LabelPill.svelte';
  import MessageBlock from '$lib/ui/MessageBlock.svelte';
  import OptionList from '$lib/ui/OptionList.svelte';
  import Button from '$lib/ui/Button.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Toggle from '$lib/ui/Toggle.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import AskPanel from '$lib/ui/AskPanel.svelte';
  import PermissionRow from '$lib/ui/PermissionRow.svelte';
  import QuestionPrompt from '$lib/ui/QuestionPrompt.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import RestMark from '$lib/ui/RestMark.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import WorkLine from '$lib/ui/WorkLine.svelte';
  import SearchField from '$lib/ui/SearchField.svelte';
  import SignInPanel from '$lib/ui/SignInPanel.svelte';
  import SessionRow from '$lib/ui/SessionRow.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import ShotChip from '$lib/ui/ShotChip.svelte';
  import ShotPreview from '$lib/ui/ShotPreview.svelte';
  import ShotPrompt from '$lib/ui/ShotPrompt.svelte';
  import TaskRow from '$lib/ui/TaskRow.svelte';
  import Toast from '$lib/ui/Toast.svelte';
  import UsageBar from '$lib/ui/UsageBar.svelte';
  import UsageCorner from '$lib/ui/UsageCorner.svelte';
  import UsageDial from '$lib/ui/UsageDial.svelte';
  import { effortOptions, modelOptions } from '$lib/logic/agent';
  import { ELSEWHERE_NOTE, FINISHED_NOTE, noteTitle } from '$lib/logic/agent';
  import AgentBar from '$lib/ui/AgentBar.svelte';
  import NoteBlock from '$lib/ui/NoteBlock.svelte';
  import PickerMenu from '$lib/ui/PickerMenu.svelte';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';
  import type { PromptRequest } from '$lib/types/generated/PromptRequest';
  import type { AgentSetup } from '$lib/types/generated/AgentSetup';
  import type { ModelChoice } from '$lib/types/generated/ModelChoice';
  import type { SessionCard } from '$lib/types/generated/SessionCard';
  import type { TaskItem } from '$lib/types/generated/TaskItem';
  import type { TaskLabel } from '$lib/types/generated/TaskLabel';
  import type { FeedEntry } from '$lib/types/generated/FeedEntry';
  import type { IslandView } from '$lib/types/generated/IslandView';
  import type { TaskStatus } from '$lib/types/generated/TaskStatus';

  const NOW = 1_700_000_000;

  // The sink has no shots cache to read, so the tile gets a picture of its
  // own rather than a path that resolves to nothing.
  const SHOT =
    'data:image/svg+xml,' +
    encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" width="112" height="80">' +
        '<rect width="112" height="80" fill="rgb(38,42,50)"/>' +
        '<rect x="8" y="10" width="58" height="8" rx="4" fill="rgb(125,216,143)"/>' +
        '<rect x="8" y="26" width="92" height="6" rx="3" fill="rgb(88,94,108)"/>' +
        '<rect x="8" y="40" width="74" height="6" rx="3" fill="rgb(88,94,108)"/>' +
        '</svg>',
    );

  const options: ChoiceOption[] = [
    // A trailing "(Recommended)" in the label is the badge, the same
    // convention `AskUserQuestion` marks its own pick with. No extra field.
    { id: 'allow_once', label: 'Allow once (Recommended)', hint: null, kind: 'AllowOnce' },
    { id: 'allow_always', label: 'Allow for this session', hint: null, kind: 'AllowAlways' },
    { id: 'deny', label: 'Deny', hint: 'Type a reason first', kind: 'Deny' },
  ];

  const labels: TaskLabel[] = ['Code', 'Fix', 'Bug', 'Test', 'Docs', 'Chore', 'Research'];
  const statuses: TaskStatus[] = ['Pending', 'Active', 'Done'];

  const tasks: TaskItem[] = statuses.map((status, index) => ({
    id: String(index),
    title: `A task title long enough to need an ellipsis somewhere around here (${status})`,
    label: labels[index],
    status,
    session_id: 's',
    updated_at: 0,
  }));

  const longMessage = Array.from(
    { length: 12 },
    (_, i) => `Line ${i + 1} of a closing message that runs past the six line clamp.`,
  ).join('\n');

  // Every IslandView, because the spring cannot be looked at without a live
  // session otherwise. tech.md 9.
  const views: [string, IslandView][] = [
    ['Collapsed', 'Collapsed'],
    ['Pill', 'Pill'],
    ['Ask', 'Ask'],
    ['Sessions', 'Sessions'],
    ['Session', { Session: '01J0' }],
  ];
  const NOTCH = { width: 200, height: 32 };

  // Every EntryState, including the one only the Stop sweep can produce.
  // Everything a message can carry: inline code, bold, and a fenced block.
  const formatted: FeedEntry = {
    id: 'k0',
    kind: 'Assistant',
    detail: null,
    text: 'Ran `cargo test` and it is **green**.\n\n```rust\nlet ok = true;\n```',
    tool: null,
    state: 'Ok',
    at: 0,
  };

  const entries: FeedEntry[] = [
    {
      id: 'e0',
      kind: 'User',
      detail: null,
      text: 'ship the feed slice',
      tool: null,
      state: 'Ok',
      at: 0,
    },
    {
      id: 'e1',
      kind: 'Tool',
      detail: null,
      text: 'cargo test --workspace',
      tool: 'Bash',
      state: 'Running',
      at: 1,
    },
    {
      id: 'e2',
      kind: 'Tool',
      detail: null,
      text: 'crates/peekle-core/src/sessions.rs',
      tool: 'Read',
      state: 'Ok',
      at: 2,
    },
    { id: 'e3', kind: 'Tool', detail: null, text: 'exit 42', tool: 'Bash', state: 'Failed', at: 3 },
    {
      id: 'e4',
      kind: 'Assistant',
      detail: null,
      text: 'The sweep closes the row at the turn boundary.',
      tool: null,
      state: 'Ok',
      at: 4,
    },
  ];

  const questions: import('$lib/types/generated/Question').Question[] = [
    {
      header: 'Framework',
      question: 'Which framework should the new dashboard use?',
      options: [
        { label: 'SvelteKit', description: 'Already the stack everywhere else' },
        { label: 'Next.js', description: null },
      ],
      multi_select: false,
    },
    {
      header: 'Checks',
      question: 'Which checks should block the merge?',
      options: [
        { label: 'Lint', description: null },
        { label: 'Type check', description: null },
        { label: 'Tests', description: null },
      ],
      multi_select: true,
    },
  ];

  const permission: PromptRequest = {
    id: '01J0',
    kind: 'Permission',
    session: { session_id: 's', cwd: '/Users/x/peekle', project: 'peekle', pid: null, tty: null },
    title: 'Bash needs permission',
    last_message: null,
    detail: '{"command":"rm -rf target/debug/incremental"}',
    options: [
      { id: 'allow_once', label: 'Allow once', hint: null, kind: 'AllowOnce' },
      { id: 'allow_always', label: 'Allow for this session', hint: null, kind: 'AllowAlways' },
      { id: 'deny', label: 'Deny', hint: null, kind: 'Deny' },
    ],
    questions: [],
    allow_free_text: true,
    created_at: 0,
    expires_at: 0,
  };

  const modelRows: ModelChoice[] = [
    { alias: 'fable', label: 'Fable 5', id: 'claude-fable-5' },
    { alias: 'opus', label: 'Opus 5', id: 'claude-opus-5' },
    { alias: 'sonnet', label: 'Sonnet 5', id: 'claude-sonnet-5' },
    { alias: 'haiku', label: 'Haiku 4.5', id: 'claude-haiku-4-5' },
  ];
  const modelPicks = modelOptions(modelRows);
  const effortPicks = effortOptions({
    levels: ['Low', 'Medium', 'High', 'XHigh', 'Max'],
  } as AgentSetup);

  const opus: AgentSetup = {
    model: 'claude-opus-5',
    label: 'Opus 5',
    effort: 'High',
    levels: ['Low', 'Medium', 'High', 'XHigh', 'Max'],
    context_tokens: 612_000,
    context_window: 1_000_000,
    context_pct: 61.2,
  };
  // A model that takes no effort at all is not offered a dead menu. 6.15.
  const haiku: AgentSetup = {
    model: 'claude-haiku-4-5',
    label: 'Haiku 4.5',
    effort: null,
    levels: [],
    context_tokens: 24_000,
    context_window: 200_000,
    context_pct: 12,
  };

  const fresh: AgentSetup = {
    model: 'claude-opus-5',
    label: 'Opus 5',
    effort: 'Low',
    levels: ['Low', 'Medium', 'High', 'XHigh', 'Max'],
    context_tokens: 0,
    context_window: 1_000_000,
    context_pct: 0,
  };

  const cards: SessionCard[] = (['Working', 'Idle', 'Ended'] as const).map((status, i) => ({
    session: {
      session_id: `s${i}`,
      cwd: '/Users/x/peekle',
      project: 'peekle',
      pid: null,
      tty: null,
    },
    title: 'Refactor the panel code and open a PR when the tests pass',
    status,
    origin: i === 0 ? 'Owned' : 'Observed',
    entries: [],
    agent: null,
    mode: null,
    thinking: null,
    updated_at: 0,
  }));

  let view = $state<IslandView>('Pill');
  let text = $state('');
  let selected = $state('allow_once');
  let collapsed = $state(true);
  let search = $state('');
</script>

<div class="sink">
  <section>
    <h2>Shape</h2>
    <div class="views">
      {#each views as [name, candidate] (name)}
        <button
          class="pick"
          class:on={JSON.stringify(view) === JSON.stringify(candidate)}
          onclick={() => (view = candidate)}>{name}</button
        >
      {/each}
    </div>
    <div class="stage">
      <Shape {view} notch={NOTCH}>
        {#snippet rest()}
          <RestMark status="waiting" pct={62} onopen={() => {}} />
        {/snippet}
        <Toast text="Claude needs your input" tone="Neutral" badge={3} />
      </Shape>
    </div>

    <div class="row">
      {#each views as [name, candidate] (name)}
        <div class="stage small">
          <Shape view={candidate} notch={NOTCH}>
            <Toast text={name} tone="Neutral" />
          </Shape>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>RestMark</h2>
    <!-- Drawn on the island fill, because that is the only surface it ever
         appears on and any other background lies about the contrast. -->
    <div class="row">
      {#each ['idle', 'working', 'waiting'] as const as status (status)}
        <div class="mark-stage">
          <RestMark {status} pct={12} onopen={() => {}} />
        </div>
      {/each}
      <!-- The badge as it stands on a ten, in the room the shape makes for it.
           tech.md 6.18. -->
      <div class="mark-stage wide">
        <RestMark status="idle" pct={70} badge={70} onopen={() => {}} />
      </div>
    </div>
    <!-- Every threshold of section 9, plus the unknown that draws an empty
         ring rather than a zero. tech.md R-3. -->
    <div class="row">
      {#each [null, 12, 62, 81, 96] as pct (String(pct))}
        <div class="mark-stage">
          <RestMark status="idle" {pct} onopen={() => {}} />
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Message formatting</h2>
    <!-- The three things a turn actually uses, on the island's own black. -->
    <div class="stage messages">
      <FeedRow entry={formatted} />
      <FeedRow entry={{ ...formatted, id: 'k1', kind: 'User', text: 'ship it' }} />
      <!-- Something the conversation records that nobody said. tech.md 6.15. -->
      <FeedRow
        entry={{ ...formatted, id: 'k2', kind: 'Notice', text: 'Switched to claude-opus-5[1m]' }}
      />
    </div>
  </section>

  <section>
    <h2>SearchField</h2>
    <div class="frame"><SearchField bind:value={search} /></div>

    <h2>SignInPanel</h2>
    <div class="frame">
      <SignInPanel
        signIn={{ stage: 'Idle', url: null, needs_code: false, error: null }}
        reason="Offline"
      />
    </div>
    <div class="frame">
      <SignInPanel
        signIn={{ stage: 'Idle', url: null, needs_code: false, error: null }}
        reason="NotLoggedIn"
      />
    </div>
    <div class="frame">
      <SignInPanel
        signIn={{
          stage: 'Waiting',
          url: 'https://claude.com/cai/oauth/authorize?code=true',
          needs_code: true,
          error: null,
        }}
        reason="NotLoggedIn"
      />
    </div>
    <div class="frame">
      <SignInPanel
        signIn={{
          stage: 'Failed',
          url: null,
          needs_code: false,
          error: 'That code was not accepted.',
        }}
        reason="NotLoggedIn"
      />
    </div>

    <h2>SignInPanel · strip</h2>
    <!-- The strip under the session list: the same panel in one column, with
         everything a press can produce landing somewhere. tech.md 6.16. -->
    <div class="frame">
      <SignInPanel
        compact
        signIn={{ stage: 'Idle', url: null, needs_code: false, error: null }}
        reason="NotLoggedIn"
      />
    </div>
    <div class="frame">
      <SignInPanel
        compact
        signIn={{
          stage: 'Refused',
          url: null,
          needs_code: false,
          error: 'Check your connection or VPN, then wait a moment.',
        }}
        reason="NotLoggedIn"
      />
    </div>
    <div class="frame">
      <SignInPanel
        compact
        signIn={{ stage: 'Waiting', url: 'https://claude.ai/x', needs_code: true, error: null }}
      />
    </div>

    <h2>UsageCorner</h2>
    <!-- The top right corner of a dialogue: both windows and the context, the
         last of them a button. tech.md 6.12 and 6.15. -->
    <div class="frame">
      <UsageCorner hour={42} week={68} />
    </div>

    <h2>AgentBar</h2>
    <!-- The row as the composer carries it: ring, model with its weight,
         thinking, and the mode beside the send button. tech.md 6.15. -->
    <div class="frame">
      <AgentBar
        agent={opus}
        models={modelRows}
        live
        canPickMode
        canUltra
        mode="Auto"
        thinking={true}
        contextTitle="56% of context used. Click to compact."
        onmodel={() => {}}
        oneffort={() => {}}
        onmode={() => {}}
      />
    </div>
    <!-- On ultracode, in the original's own colour. -->
    <div class="frame">
      <AgentBar
        agent={opus}
        models={modelRows}
        live
        canPickMode
        canUltra
        ultra
        mode="Plan"
        thinking={false}
        contextTitle="56% of context used. Click to compact."
      />
    </div>
    <!-- A model that takes no effort at all, and a session that has not
         answered yet: thinking can still be set, ultracode cannot. -->
    <div class="frame">
      <AgentBar agent={haiku} models={modelRows} live canPickMode mode="Manual" />
    </div>
    <div class="frame">
      <AgentBar
        agent={null}
        defaults={fresh}
        models={modelRows}
        live
        canPickMode
        canSetThinking
        thinking={false}
        mode="Plan"
      />
    </div>
    <!-- A chat another app runs: reading, with the note saying where the
         setting lives. -->
    <div class="frame">
      <AgentBar agent={opus} models={modelRows} note={noteTitle(ELSEWHERE_NOTE)} />
    </div>
  </section>

  <section>
    <h2>NoteBlock</h2>
    <!-- The answer a pressed reading gives: what is so, and the way out.
         tech.md 6.15. -->
    <NoteBlock fact={ELSEWHERE_NOTE.fact} how={ELSEWHERE_NOTE.how} />
    <NoteBlock fact={FINISHED_NOTE.fact} />
  </section>

  <section>
    <h2>IconButton and Toggle</h2>
    <!-- The gear that opens settings, and the one setting under it.
         tech.md 6.17. -->
    <div class="row">
      <IconButton name="settings" title="Settings" onclick={() => {}} />
      <IconButton name="settings" title="Settings" pressed onclick={() => {}} />
      <IconButton name="back" title="Back to the session list" onclick={() => {}} />
    </div>
    <Toggle
      label="Notify when a turn ends"
      hint="macOS decides whether banners appear. Check System Settings › Notifications."
      checked={notifyOn}
      onchange={(next) => (notifyOn = next)}
    />
    <Toggle label="Waiting on the system" checked={false} busy />
  </section>

  <section>
    <h2>PickerMenu</h2>
    <div class="row">
      <PickerMenu label="Opus 5" options={modelPicks} value="opus" onpick={() => {}} />
      <PickerMenu label="High" options={effortPicks} value="High" pending onpick={() => {}} />
      <PickerMenu label="Opus 5" options={modelPicks} disabled onpick={() => {}} />
    </div>
  </section>

  <section>
    <h2>UsageDial</h2>
    <div class="stage messages dials">
      <UsageDial pct={null} label="5h" />
      <UsageDial pct={18} label="5h" />
      <UsageDial pct={64} label="5h" />
      <UsageDial pct={82} label="7d" />
      <UsageDial pct={97} label="7d" />
    </div>
  </section>

  <section>
    <h2>Objects with a body</h2>
    <!-- A tool call and a thought, collapsed as the terminal shows them. -->
    <div class="stage messages">
      <FeedRow
        entry={{
          id: 'o0',
          kind: 'Tool',
          tool: 'Bash',
          text: 'cargo test --workspace',
          detail: '{\n  "command": "cargo test --workspace"\n}\n\ntest result: ok. 54 passed',
          state: 'Ok',
          at: 0,
        }}
      />
      <FeedRow
        entry={{
          id: 'o1',
          kind: 'Thought',
          tool: null,
          text: 'Thought for 12s',
          detail: 'weighing two options',
          state: 'Ok',
          at: 0,
        }}
      />
    </div>
  </section>

  <section>
    <h2>WorkLine</h2>
    <!-- Both halves of a run: the clock while it goes, the span once it is
         over. tech.md 6.12. -->
    <div class="stage messages">
      <WorkLine running from={Date.now() - 42_000} />
      <WorkLine running={false} from={0} to={74_000} />
    </div>
  </section>

  <section>
    <h2>ShotPrompt</h2>
    <!-- The pill as it stands through its five seconds. tech.md 6.13. -->
    <div class="pill"><ShotPrompt project="peekle" left={1} /></div>
    <div class="pill"><ShotPrompt project="a-long-project-name" left={0.4} secs={2} /></div>
    <div class="pill"><ShotPrompt project="peekle" left={0} /></div>
  </section>

  <section>
    <h2>ShotChip</h2>
    <div class="frame row">
      <ShotChip name="01JBQ8WMKX.png" src={SHOT} />
      <ShotChip name="a-screenshot-with-a-very-long-name-indeed.png" />
    </div>
  </section>

  <section>
    <h2>ShotPreview</h2>
    <div class="frame preview"><ShotPreview name="01JBQ8WMKX.png" src={SHOT} /></div>
  </section>

  <section>
    <h2>PromptInput</h2>
    <!-- The composer as the island builds it: the text, and under it the
         controls of the message being written. tech.md 6.15. -->
    <div class="frame">
      <PromptInput bind:value={text} placeholder="Reply to Claude">
        {#snippet tools()}
          <AgentBar
            agent={opus}
            models={modelRows}
            live
            canPickMode
            mode="Auto"
            onmodel={() => {}}
            oneffort={() => {}}
            onmode={() => {}}
          />
        {/snippet}
      </PromptInput>
    </div>
    <div class="frame"><PromptInput value="disabled" disabled /></div>
    <!-- Mid turn the one button becomes the way to end it: a white square on
         the product's green, where the arrow was. tech.md 6.5. -->
    <div class="frame">
      <PromptInput placeholder="Message Claude" working onstop={() => {}} />
    </div>
  </section>

  <section>
    <h2>OptionList</h2>
    <div class="frame"><OptionList {options} bind:selected /></div>
  </section>

  <section>
    <h2>MessageBlock</h2>
    <div class="frame"><MessageBlock text={longMessage} bind:collapsed /></div>
    <div class="frame"><MessageBlock text="Short closing message." collapsed={false} /></div>
  </section>

  <section>
    <h2>UsageBar</h2>
    <div class="frame">
      <UsageBar label="5h" pct={12} resetsAt={NOW + 5400} now={NOW} />
      <UsageBar label="5h" pct={62} resetsAt={NOW + 600} now={NOW} />
      <UsageBar label="Week" pct={81} resetsAt={NOW + 300000} now={NOW} />
      <UsageBar label="Week" pct={96} resetsAt={NOW + 30} now={NOW} />
      <UsageBar label="5h" pct={null} reason="No Keychain access" />
      <UsageBar label="Week" pct={null} reason="Not logged in" />
    </div>
  </section>

  <section>
    <h2>TaskRow and LabelPill</h2>
    <div class="frame">
      {#each tasks as task (task.id)}
        <TaskRow {task} />
      {/each}
    </div>
    <div class="frame">
      {#each labels as label (label)}
        <LabelPill {label} />
      {/each}
    </div>
  </section>

  <section>
    <h2>FeedRow</h2>
    <div class="frame">
      {#each entries as entry (entry.id)}
        <FeedRow {entry} />
      {/each}
    </div>
  </section>

  <section>
    <h2>SessionRow</h2>
    <div class="frame">
      {#each cards as card (card.session.session_id)}
        <SessionRow {card} />
      {/each}
    </div>
  </section>

  <section>
    <h2>PermissionRow</h2>
    <div class="frame"><PermissionRow request={permission} /></div>
  </section>

  <section>
    <h2>AskPanel</h2>
    <!-- On the island fill and at the width the Ask view springs to, because
         that is the only place it is ever seen. tech.md 6.7. -->
    <!-- Fresh, so the clock and the hairline are the live ones. -->
    <div class="ask-stage">
      <AskPanel request={{ ...permission, created_at: Date.now() }} />
    </div>
  </section>

  <section>
    <h2>QuestionPrompt</h2>
    <!-- AskUserQuestion answered one question at a time: single-select
         advances on click, multiSelect needs its own Submit. tech.md 6.14. -->
    <div class="frame"><QuestionPrompt {questions} /></div>
  </section>

  <section>
    <h2>Button</h2>
    <div class="frame choices">
      <Button label="Continue" variant="primary" />
      <Button label="Connect" variant="connect" />
      <Button label="Connecting" variant="connect" disabled />
      <Button label="Connect" variant="connect" wide />
      <Button label="Finish" />
      <Button label="Finish" disabled />
    </div>
  </section>

  <section>
    <h2>ScrollHint</h2>
    <div class="frame"><ScrollHint visible /></div>
    <div class="frame"><ScrollHint visible={false} /></div>
  </section>

  <section>
    <h2>Toast</h2>
    <div class="pill"><Toast text="Peekle is ON" tone="On" /></div>
    <div class="pill"><Toast text="Peekle is OFF" tone="Off" /></div>
    <div class="pill"><Toast text="Hotkey is taken" tone="Warn" /></div>
    <div class="pill"><Toast text="Claude needs your input" tone="Neutral" badge={3} /></div>
  </section>

  <section>
    <h2>Kbd</h2>
    <div class="frame">
      <Kbd keys={['⌥', '⇧', 'Q']} />
      <Kbd keys={['esc']} />
      <Kbd keys={['⏎']} />
    </div>
  </section>
</div>

<style>
  /* Dev-only surface for visual regression, so an opaque ground is fine here
     and nowhere else. */
  .sink {
    background: #1b2430;
    min-height: 100vh;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 28px;
    overflow-y: auto;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 720px;
  }

  h2 {
    margin: 0;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
    font-weight: 500;
  }

  .pill {
    height: 44px;
    width: 340px;
    background: var(--notch);
    border-radius: 0 0 20px 20px;
  }

  /* Dev-only framing so each primitive has an edge to be seen against. The
     product draws its surfaces with Shape. */
  .frame {
    background: var(--surface);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 12px 14px;
  }

  /* The preview is a layer over content, so the sink gives it something to
     lie over rather than showing it against the page. */
  .preview {
    position: relative;
    height: 200px;
  }

  .views {
    display: flex;
    gap: 6px;
  }

  .choices {
    display: flex;
    gap: 6px;
  }

  .pick {
    background: transparent;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    color: var(--text-dim);
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
  }

  .pick.on {
    color: var(--text);
    border-color: var(--accent);
  }

  /* The window is 720 by 560 and the shape hangs from its top edge. */
  .stage {
    width: 720px;
    height: 200px;
    max-width: 100%;
    background: repeating-linear-gradient(45deg, #22303f, #22303f 10px, #1f2b38 10px, #1f2b38 20px);
    overflow: hidden;
  }

  .stage.small {
    width: 170px;
    height: 120px;
  }

  /* The mark only ever sits on the island fill, so anything else here would
     lie about its contrast. */
  .dials {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .messages {
    background: var(--notch);
    padding: 10px;
    height: auto;
  }

  .ask-stage {
    width: 460px;
    height: 62px;
    overflow: hidden;
    background: var(--notch);
    border-radius: 0 0 22px 22px;
  }

  .mark-stage {
    width: 96px;
    height: 26px;
    background: var(--notch);
    border-radius: 0 0 10px 10px;
  }

  /* The width the shape springs to while the badge stands: 2 * REST_BADGE
     wider than the mark beside it. tech.md 6.18. */
  .mark-stage.wide {
    width: 168px;
  }

  .row {
    display: flex;
    gap: 8px;
  }
</style>
