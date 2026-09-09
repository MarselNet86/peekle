<script lang="ts">
  /**
   * Model and effort as one block, the way the original's composer carries
   * them: the name, the weight beside it in grey, and one menu holding both —
   * the models with a line each, and the effort as a track under them.
   * tech.md 6.15.
   */
  import {
    askedLabel,
    currentModel,
    effortHint,
    effortLabel,
    effortOptions,
    modelLabel,
    modelOptions,
    ULTRACODE,
    ULTRACODE_HINT,
  } from '$lib/logic/agent';
  import type { AgentSetup } from '$lib/types/generated/AgentSetup';
  import type { Effort } from '$lib/types/generated/Effort';
  import type { ModelChoice } from '$lib/types/generated/ModelChoice';

  let {
    agent,
    models = [],
    live = false,
    ultra = false,
    canUltra = false,
    askedModel = null,
    askedEffort = null,
    pendingModel = false,
    pendingEffort = false,
    note = '',
    onmodel,
    oneffort,
    onultra,
    onnote,
  }: {
    agent: AgentSetup | null;
    models?: ModelChoice[];
    live?: boolean;
    /** Whether this session was put on ultracode. Held locally: the file
     * records `xhigh`, because that is what ultracode runs at. 6.15. */
    ultra?: boolean;
    /** Ultracode holds for a running session only, which is what the CLI
     * itself says of it, so it is not offered before there is one. */
    canUltra?: boolean;
    askedModel?: string | null;
    askedEffort?: Effort | null;
    pendingModel?: boolean;
    pendingEffort?: boolean;
    note?: string;
    onmodel?: (alias: string) => void;
    oneffort?: (effort: Effort) => void;
    onultra?: () => void;
    onnote?: () => void;
  } = $props();

  let open = $state(false);
  let host = $state<HTMLElement | null>(null);

  const rows = $derived(modelOptions(models));
  const levels = $derived(effortOptions(agent).map((row) => row.id as Effort));
  const picked = $derived(askedModel ?? currentModel(agent, models));
  const shownEffort = $derived(askedEffort ?? agent?.effort ?? null);
  // While a pick travels the block stands on it: the choice is the freshest
  // true thing about the session, even before it applies. tech.md 6.15.
  const name = $derived(askedLabel(askedModel, models) || modelLabel(agent) || 'Model');
  const weight = $derived(ultra ? 'Ultracode' : effortLabel(shownEffort));

  /** Where the knob stands: the level's place on the track, ultracode past
   * the end of it. */
  const stops = $derived(canUltra ? [...levels.map(String), ULTRACODE] : levels.map(String));
  const at = $derived(stops.indexOf(ultra ? ULTRACODE : String(shownEffort ?? '')));

  function toggle() {
    if (!live) {
      onnote?.();
      return;
    }
    open = !open;
  }

  function pickModel(alias: string) {
    open = false;
    onmodel?.(alias);
  }

  function pickStop(stop: string) {
    if (stop === ULTRACODE) onultra?.();
    else oneffort?.(stop as Effort);
  }

  function away(event: MouseEvent) {
    if (!open || !host) return;
    if (!host.contains(event.target as Node)) open = false;
  }

  function keydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') {
      event.preventDefault();
      open = false;
    }
  }
</script>

<svelte:window onclick={away} onkeydown={keydown} />

<span class="block" bind:this={host}>
  <button
    class="chip"
    class:ultra
    class:dim={!live}
    title={live ? 'Model and effort' : note}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={toggle}
  >
    <span class="name" class:pending={pendingModel}>{name}</span>
    {#if weight}
      <span class="weight" class:pending={pendingEffort}>{weight}</span>
    {/if}
  </button>

  {#if open}
    <span class="menu" role="menu">
      <span class="head">Select a model</span>
      {#each rows as row (row.id)}
        <button
          class="option"
          role="menuitemradio"
          aria-checked={row.id === picked}
          onclick={() => pickModel(row.id)}
        >
          <span class="text">
            {row.label}
            {#if row.hint}<span class="hint">{row.hint}</span>{/if}
          </span>
          <span class="tick" aria-hidden="true">{row.id === picked ? '✓' : ''}</span>
        </button>
      {/each}

      {#if stops.length > 0}
        <!-- The effort as the original draws it: one row, the label naming
             where it stands, and a track of stops. Ultracode is the stop past
             the end, in its own colour. tech.md 6.15. -->
        <div class="effort" class:ultra>
          <span class="label">
            Effort <span class="which">({ultra ? ULTRACODE_HINT : (weight ?? '')})</span>
          </span>
          <span class="track">
            {#each stops as stop, index (stop)}
              <button
                class="stop"
                class:on={index <= at}
                class:knob={index === at}
                class:top={stop === ULTRACODE}
                title={stop === ULTRACODE ? ULTRACODE_HINT : effortHint(stop as Effort)}
                aria-label={stop}
                onclick={() => pickStop(stop)}
              ></button>
            {/each}
          </span>
        </div>
      {/if}
    </span>
  {/if}
</span>

<style>
  .block {
    position: relative;
    display: inline-flex;
  }

  /* The dark block of the original: the name lit, the weight beside it grey,
     one ground under both. tech.md 6.15. */
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    border-radius: 999px;
    background: var(--bubble);
    color: var(--text);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    padding: 5px 10px;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background 140ms ease-out,
      color 140ms ease-out;
  }

  .chip:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .chip.dim {
    color: var(--text-dim);
    cursor: default;
  }

  /* Ultracode paints the block, because it is the one setting that changes
     what a turn costs. The colour is the original's own. tech.md 6.15. */
  .chip.ultra {
    background: rgba(208, 180, 255, 0.16);
    color: var(--ultra);
  }

  .weight {
    color: var(--text-dim);
  }

  .chip.ultra .weight {
    color: var(--ultra);
    opacity: 0.75;
  }

  .name.pending,
  .weight.pending {
    opacity: 0.55;
  }

  .menu {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 0;
    z-index: 3;
    display: flex;
    flex-direction: column;
    min-width: 320px;
    padding: 6px;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    background: var(--notch);
    transform-origin: bottom left;
    /* Grown from the block that opened it. tech.md 6.10. */
    animation: grow 160ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  @keyframes grow {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .head {
    padding: 4px 8px 6px;
    font-size: 11px;
    color: var(--text-dim);
  }

  .option {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    border: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 6px 8px;
    border-radius: 8px;
    cursor: pointer;
  }

  .option:hover {
    background: var(--bubble);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  .hint {
    display: block;
    margin-top: 2px;
    font-size: 11px;
    line-height: 1.35;
    color: var(--text-dim);
  }

  .tick {
    flex: none;
    width: 12px;
    color: var(--brand);
    font-size: 11px;
    margin-top: 2px;
  }

  /* Its own row at the foot of the menu, on its own ground, because it is not
     one of the models above it. */
  .effort {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 4px;
    padding: 8px;
    border-radius: 9px;
    background: var(--bubble);
    transition: background 200ms ease-out;
  }

  .effort.ultra {
    background: rgba(208, 180, 255, 0.16);
  }

  .label {
    font-size: 12px;
    color: var(--text);
    white-space: nowrap;
  }

  .which {
    color: var(--text-dim);
  }

  .effort.ultra .label,
  .effort.ultra .which {
    color: var(--ultra);
  }

  .track {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    padding: 4px 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
  }

  .stop {
    width: 7px;
    height: 7px;
    padding: 0;
    border: none;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.25);
    cursor: pointer;
    /* The knob springs to its new stop rather than blinking there. 6.10. */
    transition:
      width 220ms cubic-bezier(0.22, 1, 0.36, 1),
      height 220ms cubic-bezier(0.22, 1, 0.36, 1),
      background 180ms ease-out;
  }

  .stop.on {
    background: var(--text);
  }

  .stop.knob {
    width: 13px;
    height: 13px;
    background: #fff;
  }

  .stop.top {
    background: rgba(208, 180, 255, 0.45);
  }

  .stop.top.on {
    background: var(--ultra);
  }

  .effort.ultra .stop.on {
    background: var(--ultra);
  }

  .effort.ultra .stop.knob {
    background: #fff;
  }

  .stop:focus,
  .stop:focus-visible,
  .chip:focus,
  .chip:focus-visible,
  .option:focus,
  .option:focus-visible {
    outline: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }

    .stop {
      transition: none;
    }
  }
</style>
