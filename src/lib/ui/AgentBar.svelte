<script lang="ts">
  /**
   * The row under the field: what answers, how hard it thinks, and how full
   * its context is. The same line Claude Code draws under its own input, in
   * the place a person asks the question it answers -- what will reply to
   * what I am typing. tech.md 6.15 and 9.
   */
  import {
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
    pendingModel = false,
    pendingEffort = false,
    pendingCompact = false,
    onmodel,
    oneffort,
    oncompact,
  }: {
    agent: AgentSetup | null;
    /** What the session runs as before it has answered once: Claude Code's
     * own defaults. The row stands on these so a session can be aimed before
     * it speaks. tech.md 6.15. */
    defaults?: AgentSetup | null;
    models?: ModelChoice[];
    /** Whether anything here can be changed: an owned session, still running.
     * An observed one has no channel at all, so its row reads and nothing
     * more. tech.md 6.15. */
    live?: boolean;
    pendingModel?: boolean;
    pendingEffort?: boolean;
    pendingCompact?: boolean;
    onmodel?: (alias: string) => void;
    oneffort?: (effort: Effort) => void;
    oncompact?: () => void;
  } = $props();

  // What the row is standing on: the transcript when there is one, the
  // defaults until then. tech.md 6.15.
  const shown = $derived(agent ?? defaults);
  // Whether anything has been measured. `null` and zero are different claims:
  // one is "nothing has been asked of the window", the other is a number.
  const measured = $derived(agent !== null);

  const modelRows = $derived<PickOption[]>(modelOptions(models));
  const effortRows = $derived<PickOption[]>(effortOptions(shown));
  const picked = $derived(currentModel(shown, models));
</script>

{#if shown}
  <div class="agent-bar">
    <PickerMenu
      label={modelLabel(shown) || 'Model'}
      options={modelRows}
      value={picked}
      disabled={!live}
      pending={pendingModel}
      onpick={(alias) => onmodel?.(alias)}
    />

    <!-- A model that takes no effort is not offered a dead menu. 6.15. -->
    {#if effortRows.length > 0}
      <PickerMenu
        label={effortLabel(shown.effort) || 'Effort'}
        options={effortRows}
        value={shown.effort ?? ''}
        disabled={!live}
        pending={pendingEffort}
        onpick={(level) => oneffort?.(level as Effort)}
      />
    {/if}

    <!-- The ring is the button, exactly as it is in Claude Code: what it
         shows is what clicking it acts on. tech.md 6.15. -->
    <button
      class="context"
      class:pending={pendingCompact}
      disabled={!live || !measured}
      title={contextLabel(agent) || 'Nothing in the context yet'}
      aria-label={contextLabel(agent) || 'Nothing in the context yet'}
      onclick={() => oncompact?.()}
    >
      <UsageDial pct={measured ? shown.context_pct : null} size={13} />
    </button>
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

  /* A compact takes minutes and confirms itself by the number falling, so the
     ring waits visibly rather than sitting as if nothing was asked. 6.15. */
  .context.pending {
    opacity: 0.5;
  }
</style>
