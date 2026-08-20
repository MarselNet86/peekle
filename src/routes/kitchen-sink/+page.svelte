<script lang="ts">
  import Kbd from '$lib/ui/Kbd.svelte';
  import LabelPill from '$lib/ui/LabelPill.svelte';
  import MessageBlock from '$lib/ui/MessageBlock.svelte';
  import OptionList from '$lib/ui/OptionList.svelte';
  import Button from '$lib/ui/Button.svelte';
  import FeedRow from '$lib/ui/FeedRow.svelte';
  import PermissionRow from '$lib/ui/PermissionRow.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import RestMark from '$lib/ui/RestMark.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import SessionRow from '$lib/ui/SessionRow.svelte';
  import Shape from '$lib/ui/Shape.svelte';
  import TaskRow from '$lib/ui/TaskRow.svelte';
  import Toast from '$lib/ui/Toast.svelte';
  import UsageBar from '$lib/ui/UsageBar.svelte';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';
  import type { PromptRequest } from '$lib/types/generated/PromptRequest';
  import type { SessionCard } from '$lib/types/generated/SessionCard';
  import type { TaskItem } from '$lib/types/generated/TaskItem';
  import type { TaskLabel } from '$lib/types/generated/TaskLabel';
  import type { FeedEntry } from '$lib/types/generated/FeedEntry';
  import type { IslandView } from '$lib/types/generated/IslandView';
  import type { TaskStatus } from '$lib/types/generated/TaskStatus';

  const NOW = 1_700_000_000;

  const options: ChoiceOption[] = [
    { id: 'allow_once', label: 'Allow once', hint: null, kind: 'AllowOnce' },
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
    ['Sessions', 'Sessions'],
    ['Session', { Session: '01J0' }],
  ];
  const NOTCH = { width: 200, height: 32 };

  // Every EntryState, including the one only the Stop sweep can produce.
  // Everything a message can carry: inline code, bold, and a fenced block.
  const formatted: FeedEntry = {
    id: 'k0',
    kind: 'Assistant',
    text: 'Ran `cargo test` and it is **green**.\n\n```rust\nlet ok = true;\n```',
    tool: null,
    state: 'Ok',
    at: 0,
  };

  const entries: FeedEntry[] = [
    { id: 'e0', kind: 'User', text: 'ship the feed slice', tool: null, state: 'Ok', at: 0 },
    {
      id: 'e1',
      kind: 'Tool',
      text: 'cargo test --workspace',
      tool: 'Bash',
      state: 'Running',
      at: 1,
    },
    {
      id: 'e2',
      kind: 'Tool',
      text: 'crates/peekle-core/src/sessions.rs',
      tool: 'Read',
      state: 'Ok',
      at: 2,
    },
    { id: 'e3', kind: 'Tool', text: 'exit 42', tool: 'Bash', state: 'Failed', at: 3 },
    {
      id: 'e4',
      kind: 'Assistant',
      text: 'The sweep closes the row at the turn boundary.',
      tool: null,
      state: 'Ok',
      at: 4,
    },
  ];

  const permission: PromptRequest = {
    id: '01J0',
    kind: 'Permission',
    session: { session_id: 's', cwd: '/Users/x/peekle', project: 'peekle' },
    title: 'Bash needs permission',
    last_message: null,
    detail: '{"command":"rm -rf target/debug/incremental"}',
    options: [
      { id: 'allow_once', label: 'Allow once', hint: null, kind: 'AllowOnce' },
      { id: 'allow_always', label: 'Allow for this session', hint: null, kind: 'AllowAlways' },
      { id: 'deny', label: 'Deny', hint: null, kind: 'Deny' },
    ],
    allow_free_text: true,
    created_at: 0,
    expires_at: 0,
  };

  const cards: SessionCard[] = (['Working', 'WaitingOnUser', 'Idle', 'Ended'] as const).map(
    (status, i) => ({
      session: { session_id: `s${i}`, cwd: '/Users/x/peekle', project: 'peekle' },
      title: 'Refactor the panel code and open a PR when the tests pass',
      status,
      entries: [],
      updated_at: 0,
    }),
  );

  let view = $state<IslandView>('Pill');
  let text = $state('');
  let selected = $state('allow_once');
  let collapsed = $state(true);
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
    </div>
  </section>

  <section>
    <h2>PromptInput</h2>
    <div class="frame"><PromptInput bind:value={text} placeholder="Reply to Claude" /></div>
    <div class="frame"><PromptInput value="disabled" disabled /></div>
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
  .messages {
    background: var(--notch);
    padding: 10px;
    height: auto;
  }

  .mark-stage {
    width: 96px;
    height: 26px;
    background: var(--notch);
    border-radius: 0 0 10px 10px;
  }

  .row {
    display: flex;
    gap: 8px;
  }
</style>
