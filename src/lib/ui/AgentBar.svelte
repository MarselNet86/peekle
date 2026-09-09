<script lang="ts">
  /**
   * The row under the field: what answers, how hard it thinks, and how full
   * its context is. The same line Claude Code draws under its own input, in
   * the place a person asks the question it answers -- what will reply to
   * what I am typing. tech.md 6.15 and 9.
   */
  import {
    askedLabel,
    effortLabel,
    modeIcon,
    modeLabel,
    modeOptions,
    noteTitle,
    MODE_NOTE,
    effortOptions,
    modelLabel,
    modelOptions,
    currentModel,
    type PickOption,
  } from '$lib/logic/agent';
  import PickerMenu from '$lib/ui/PickerMenu.svelte';
  import type { AgentSetup } from '$lib/types/generated/AgentSetup';
  import type { Effort } from '$lib/types/generated/Effort';
  import type { ModelChoice } from '$lib/types/generated/ModelChoice';
  import type { PermissionMode } from '$lib/types/generated/PermissionMode';

  let {
    agent,
    defaults = null,
    models = [],
    live = false,
    note = '',
    askedModel = null,
    askedEffort = null,
    askedMode = null,
    mode = null,
    canPickMode = false,
    pendingModel = false,
    pendingEffort = false,
    pendingMode = false,
    onmodel,
    oneffort,
    onmode,
    onmodenote,
    onnote,
  }: {
    agent: AgentSetup | null;
    /** What the session runs as before it has answered once: Claude Code's
     * own defaults. The row stands on these so a session can be aimed before
     * it speaks. tech.md 6.15. */
    defaults?: AgentSetup | null;
    models?: ModelChoice[];
    /** Whether anything here can be changed: an owned session, still running.
     * A chat another app is running takes text but not commands, so its row
     * reads and says where the settings live. tech.md 6.15. */
    live?: boolean;
    /** Why it only reads, shown on the values themselves. Empty when live. */
    note?: string;
    /** A turn is running and there is somewhere to send the stop. 6.5. */
    /** What was chosen and has not come back yet. The row stands on this
     * while it travels, so a choice shows as made. tech.md 6.15. */
    askedModel?: string | null;
    askedEffort?: Effort | null;
    askedMode?: PermissionMode | null;
    /** What the session's own hooks report it is running in. tech.md 6.19. */
    mode?: PermissionMode | null;
    /** Whether the mode can still be chosen: a session Peekle is about to
     * start. After the first answer it reads, because the flag is a spawn
     * flag and the CLI has no line for it. tech.md 6.19. */
    canPickMode?: boolean;
    pendingModel?: boolean;
    pendingEffort?: boolean;
    pendingMode?: boolean;
    onmodel?: (alias: string) => void;
    oneffort?: (effort: Effort) => void;
    onmode?: (mode: PermissionMode) => void;
    /** The mode chip was pressed on a session that is already under way.
     * A press deserves an answer, and the answer is where the switch is.
     * tech.md 6.19. */
    onmodenote?: () => void;
    /** A value that only reads was pressed anyway. A press deserves an
     * answer, and the answer is `note`. tech.md 6.15. */
    onnote?: () => void;
  } = $props();

  // What the row is standing on: the transcript when there is one, the
  // defaults until then. tech.md 6.15.
  const shown = $derived(agent ?? defaults);
  const modelRows = $derived<PickOption[]>(modelOptions(models));
  const effortRows = $derived<PickOption[]>(effortOptions(shown));
  // While a pick travels, both the label and the tick stand on it: the choice
  // is the freshest true thing about the session, even before it applies.
  const picked = $derived(askedModel ?? currentModel(shown, models));
  const modelText = $derived(askedLabel(askedModel, models) || modelLabel(shown) || 'Model');
  const effortShown = $derived(askedEffort ?? shown?.effort ?? null);
  // The mode as the session reports it, or as it was just asked for. Nothing
  // is assumed: with neither, the chip stands on the CLI's own default, which
  // is what a session with no `--permission-mode` starts in. tech.md 6.19.
  const modeShown = $derived<PermissionMode>(askedMode ?? mode ?? 'Manual');
</script>

{#if shown}
  <div class="agent-bar">
    {#if live}
      <PickerMenu
        label={modelText}
        options={modelRows}
        value={picked}
        pending={pendingModel}
        onpick={(alias) => onmodel?.(alias)}
      />

      <!-- A model that takes no effort is not offered a dead menu. 6.15. -->
      {#if effortRows.length > 0}
        <PickerMenu
          label={effortLabel(effortShown) || 'Effort'}
          options={effortRows}
          value={effortShown ?? ''}
          pending={pendingEffort}
          onpick={(level) => oneffort?.(level as Effort)}
        />
      {/if}

      <!-- Nearest the send button, because it decides what pressing it will
           be allowed to do. tech.md 6.19. -->
      {#if canPickMode}
        <PickerMenu
          label={modeLabel(modeShown)}
          options={modeOptions()}
          value={modeShown}
          icon={modeIcon(modeShown)}
          pending={pendingMode}
          onpick={(next) => onmode?.(next as PermissionMode)}
        />
      {:else}
        <button class="reading" title={noteTitle(MODE_NOTE)} onclick={() => onmodenote?.()}>
          {modeLabel(modeShown)}
        </button>
      {/if}
    {:else}
      <!-- Reading, not a control that does nothing. A dimmed menu that never
           opens is read as a broken button; plain text is read as what it is,
           and the note says where the setting lives. tech.md 6.15. -->
      <button class="reading" title={note} onclick={() => onnote?.()}>
        {modelLabel(shown) || 'Model'}
      </button>
      {#if shown.effort}
        <button class="reading" title={note} onclick={() => onnote?.()}>
          {effortLabel(shown.effort)}
        </button>
      {/if}
      <button class="reading" title={note} onclick={() => onnote?.()}>
        {modeLabel(modeShown)}
      </button>
    {/if}
  </div>
{/if}

<style>
  /* Inside the field's own capsule, under the text: these are the controls of
     the message being written, not a line about the session. The composer of
     Claude Code is laid out the same way. tech.md 6.15. */
  .agent-bar {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }

  /* The same metrics as a menu button with the affordance taken off: no
     chevron, no hover, and a cursor that promises nothing. It still takes a
     press, because a value that looks like a value gets pressed anyway, and
     silence is the worst possible answer to that. tech.md 6.15. */
  /* A value that only reads, drawn as one. With the chevron gone (9) this is
     what separates it from a value that opens a menu: that one is lit and
     lights its own ground on hover, this one is quiet and does neither. */
  .reading {
    border: none;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    padding: 3px 6px;
    white-space: nowrap;
    cursor: default;
  }
</style>
