<script lang="ts">
  /**
   * The row inside the field's capsule: the model with its effort, whether it
   * thinks, and — at the right edge, beside the send button — the context ring
   * and the mode. The right edge is what decides the next turn: the ring
   * compacts, the mode says what the press is allowed to do, and both are
   * reached for before sending rather than after. What the row starts with is
   * not here at all: the plus that attaches to the message is the composer's
   * (6.25). tech.md 6.15, 6.19 and 6.20.
   */
  import { modeIcon, modeLabel, modeOptions, noteTitle, MODE_NOTE } from '$lib/logic/agent';
  import ContextRing from '$lib/ui/ContextRing.svelte';
  import ModelBlock from '$lib/ui/ModelBlock.svelte';
  import PickerMenu from '$lib/ui/PickerMenu.svelte';
  import ThinkingChip from '$lib/ui/ThinkingChip.svelte';
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
    contextTitle = '',
    mode = null,
    canPickMode = false,
    thinking = null,
    canSetThinking = false,
    ultra = false,
    canUltra = false,
    askedModel = null,
    askedEffort = null,
    askedMode = null,
    pendingModel = false,
    pendingEffort = false,
    pendingMode = false,
    pendingCompact = false,
    onmodel,
    oneffort,
    onultra,
    oncompact,
    onmode,
    onmodenote,
    onthinking,
    onnote,
  }: {
    agent: AgentSetup | null;
    /** What the session runs as before it has answered once. tech.md 6.15. */
    defaults?: AgentSetup | null;
    models?: ModelChoice[];
    /** Whether anything here can be changed: a session the island owns. */
    live?: boolean;
    /** Why it only reads, shown on the values themselves. Empty when live. */
    note?: string;
    contextTitle?: string;
    mode?: PermissionMode | null;
    canPickMode?: boolean;
    /** Whether this session runs with thinking on. Null when nobody knows,
     * which is every session Peekle did not start. tech.md 6.20. */
    thinking?: boolean | null;
    canSetThinking?: boolean;
    ultra?: boolean;
    canUltra?: boolean;
    askedModel?: string | null;
    askedEffort?: Effort | null;
    askedMode?: PermissionMode | null;
    pendingModel?: boolean;
    pendingEffort?: boolean;
    pendingMode?: boolean;
    pendingCompact?: boolean;
    onmodel?: (alias: string) => void;
    oneffort?: (effort: Effort) => void;
    onultra?: () => void;
    oncompact?: () => void;
    onmode?: (mode: PermissionMode) => void;
    onmodenote?: () => void;
    onthinking?: (on: boolean) => void;
    /** A value that only reads was pressed anyway. A press deserves an
     * answer, and the answer is `note`. tech.md 6.15. */
    onnote?: () => void;
  } = $props();

  // What the row is standing on: the transcript when there is one, the
  // defaults until then. tech.md 6.15.
  const shown = $derived(agent ?? defaults);
  const measured = $derived(agent !== null);
  const modeShown = $derived<PermissionMode>(askedMode ?? mode ?? 'Manual');
</script>

{#if shown}
  <div class="agent-bar">
    <ModelBlock
      agent={shown}
      {models}
      {live}
      {ultra}
      {canUltra}
      {note}
      {askedModel}
      {askedEffort}
      {pendingModel}
      {pendingEffort}
      onmodel={(alias) => onmodel?.(alias)}
      oneffort={(level) => oneffort?.(level)}
      onultra={() => onultra?.()}
      onnote={() => onnote?.()}
    />

    <ThinkingChip
      on={thinking ?? true}
      live={canSetThinking}
      note={canSetThinking ? '' : 'Thinking is set when a session starts'}
      onchange={(next) => onthinking?.(next)}
      onnote={() => onnote?.()}
    />

    <span class="gap"></span>

    <!-- Beside the mode rather than at the far end of the row: what it shows
         is about the window already spent, but what it does is a compact,
         which is preparing the next turn. tech.md 6.15. -->
    <ContextRing
      pct={measured ? shown.context_pct : null}
      title={contextTitle}
      live={live && measured}
      pending={pendingCompact}
      onclick={() => (live ? oncompact?.() : onnote?.())}
    />

    <!-- Nearest the send button, because it decides what pressing it will be
         allowed to do. tech.md 6.19. -->
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
  </div>
{/if}

<style>
  .agent-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    width: 100%;
  }

  /* Everything before it sits at the left edge, the mode at the right, next
     to the button it qualifies. */
  .gap {
    flex: 1;
    min-width: 4px;
  }

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

  .reading:focus,
  .reading:focus-visible {
    outline: none;
  }
</style>
