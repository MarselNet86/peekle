<script lang="ts">
  import Kbd from '$lib/ui/Kbd.svelte';
  import LabelPill from '$lib/ui/LabelPill.svelte';
  import MessageBlock from '$lib/ui/MessageBlock.svelte';
  import OptionList from '$lib/ui/OptionList.svelte';
  import Panel from '$lib/ui/Panel.svelte';
  import PromptInput from '$lib/ui/PromptInput.svelte';
  import ScrollHint from '$lib/ui/ScrollHint.svelte';
  import TaskRow from '$lib/ui/TaskRow.svelte';
  import Toast from '$lib/ui/Toast.svelte';
  import UsageBar from '$lib/ui/UsageBar.svelte';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';
  import type { TaskItem } from '$lib/types/generated/TaskItem';
  import type { TaskLabel } from '$lib/types/generated/TaskLabel';
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

  let text = $state('');
  let selected = $state('allow_once');
  let collapsed = $state(true);
</script>

<div class="sink">
  <section>
    <h2>Panel</h2>
    <Panel tone="prompt">prompt tone</Panel>
    <Panel tone="hud">hud tone</Panel>
    <Panel tone="island">island tone</Panel>
  </section>

  <section>
    <h2>PromptInput</h2>
    <Panel><PromptInput bind:value={text} placeholder="Reply to Claude" /></Panel>
    <Panel><PromptInput value="disabled" disabled /></Panel>
  </section>

  <section>
    <h2>OptionList</h2>
    <Panel><OptionList {options} bind:selected /></Panel>
  </section>

  <section>
    <h2>MessageBlock</h2>
    <Panel><MessageBlock text={longMessage} bind:collapsed /></Panel>
    <Panel><MessageBlock text="Short closing message." collapsed={false} /></Panel>
  </section>

  <section>
    <h2>UsageBar</h2>
    <Panel>
      <UsageBar label="5h" pct={12} resetsAt={NOW + 5400} now={NOW} />
      <UsageBar label="5h" pct={62} resetsAt={NOW + 600} now={NOW} />
      <UsageBar label="Week" pct={81} resetsAt={NOW + 300000} now={NOW} />
      <UsageBar label="Week" pct={96} resetsAt={NOW + 30} now={NOW} />
      <UsageBar label="5h" pct={null} reason="No Keychain access" />
      <UsageBar label="Week" pct={null} reason="Not logged in" />
    </Panel>
  </section>

  <section>
    <h2>TaskRow and LabelPill</h2>
    <Panel tone="hud">
      {#each tasks as task (task.id)}
        <TaskRow {task} />
      {/each}
    </Panel>
    <Panel>
      {#each labels as label (label)}
        <LabelPill {label} />
      {/each}
    </Panel>
  </section>

  <section>
    <h2>ScrollHint</h2>
    <Panel tone="hud"><ScrollHint visible /></Panel>
    <Panel tone="hud"><ScrollHint visible={false} /></Panel>
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
    <Panel><Kbd keys={['⌥', '⇧', 'Q']} /> <Kbd keys={['esc']} /> <Kbd keys={['⏎']} /></Panel>
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
  }
</style>
