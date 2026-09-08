<script lang="ts">
  /**
   * The row under the field: what answers, how hard it thinks, and how full
   * its context is. The same line Claude Code draws under its own input, in
   * the place a person asks the question it answers -- what will reply to
   * what I am typing. tech.md 6.15 and 9.
   */
  import {
    askedLabel,
    contextLabel,
    effortLabel,
    effortOptions,
    modelLabel,
    modelOptions,
    currentModel,
    type PickOption,
  } from '$lib/logic/agent';
  import PickerMenu from '$lib/ui/PickerMenu.svelte';
  import UsageDial from '$lib/ui/UsageDial.svelte';
  import type { AgentSetup } from '$lib/types/generated/AgentSetup';
  import type { Effort } from '$lib/types/generated/Effort';
  import type { ModelChoice } from '$lib/types/generated/ModelChoice';

  let {
    agent,
    defaults = null,
    models = [],
    live = false,
    note = '',
    askedModel = null,
    askedEffort = null,
    pendingModel = false,
    pendingEffort = false,
    pendingCompact = false,
    onmodel,
    oneffort,
    oncompact,
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
    pendingModel?: boolean;
    pendingEffort?: boolean;
    pendingCompact?: boolean;
    onmodel?: (alias: string) => void;
    oneffort?: (effort: Effort) => void;
    oncompact?: () => void;
    /** A value that only reads was pressed anyway. A press deserves an
     * answer, and the answer is `note`. tech.md 6.15. */
    onnote?: () => void;
  } = $props();

  // What the row is standing on: the transcript when there is one, the
  // defaults until then. tech.md 6.15.
  const shown = $derived(agent ?? defaults);
  // Whether anything has been measured. `null` and zero are different claims:
  // one is "nothing has been asked of the window", the other is a number.
  const measured = $derived(agent !== null);

  const modelRows = $derived<PickOption[]>(modelOptions(models));
  const effortRows = $derived<PickOption[]>(effortOptions(shown));
  // While a pick travels, both the label and the tick stand on it: the choice
  // is the freshest true thing about the session, even before it applies.
  const picked = $derived(askedModel ?? currentModel(shown, models));
  const modelText = $derived(askedLabel(askedModel, models) || modelLabel(shown) || 'Model');
  const effortShown = $derived(askedEffort ?? shown?.effort ?? null);
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

      <!-- The ring is the button, exactly as it is in Claude Code: what it
           shows is what clicking it acts on. tech.md 6.15. -->
      <button
        class="context"
        class:pending={pendingCompact}
        disabled={!measured}
        title={contextLabel(agent) || 'Nothing in the context yet'}
        aria-label={contextLabel(agent) || 'Nothing in the context yet'}
        onclick={() => oncompact?.()}
      >
        <UsageDial pct={measured ? shown.context_pct : null} size={13} />
      </button>
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
      <button
        class="context"
        title={contextLabel(agent, false) || 'Nothing in the context yet'}
        aria-label={contextLabel(agent, false) || 'Nothing in the context yet'}
        onclick={() => onnote?.()}
      >
        <UsageDial pct={measured ? shown.context_pct : null} size={13} />
      </button>
    {/if}
  </div>
{/if}

<style>
  /* On the divider above the field: the question it answers -- what will
     reply to what I am about to type -- is asked before the reply, so it has
     to be seen before one is typed. tech.md 6.15. */
  .agent-bar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
    padding: 0 2px 8px;
  }

  /* Ending the turn is the one action in this strip that is not a setting, so
     it sits at the other end of it, away from the three that are. */

  /* The same metrics as a menu button with the affordance taken off: no
     chevron, no hover, and a cursor that promises nothing. It still takes a
     press, because a value that looks like a value gets pressed anyway, and
     silence is the worst possible answer to that. tech.md 6.15. */
  .reading {
    border: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    padding: 3px 6px;
    white-space: nowrap;
    cursor: default;
  }

  .context {
    display: inline-flex;
    align-items: center;
    border: none;
    background: transparent;
    padding: 3px 4px;
    border-radius: 999px;
    cursor: pointer;
  }

  .context:hover:not(:disabled) {
    background: var(--bubble);
  }

  .context:disabled {
    cursor: default;
  }

  /* A ring that cannot compact is a reading like the two values beside it. */
  .agent-bar:has(.reading) .context {
    cursor: default;
  }

  .agent-bar:has(.reading) .context:hover {
    background: transparent;
  }

  /* A compact takes minutes and confirms itself by the number falling, so the
     ring waits visibly rather than sitting as if nothing was asked. 6.15. */
  .context.pending {
    opacity: 0.5;
  }
</style>
