<script lang="ts">
  import { reachCopy } from '$lib/logic/reach';
  import { SIGN_BOX, SIGN_STROKES, SIGN_WEIGHT } from '$lib/logic/sign';
  import Button from './Button.svelte';
  import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

  let {
    reason = null,
    busy = false,
    compact = false,
    onretry,
  }: {
    /** Why Anthropic is out of reach. Only `Offline` and `Network` draw
     * anything: signing in is the sign-in window's. tech.md 6.16. */
    reason?: UsageUnavailable | null;
    busy?: boolean;
    /** The same panel in one column, for the strip under the session list. */
    compact?: boolean;
    onretry?: () => void;
  } = $props();

  const copy = $derived(reachCopy(reason));
</script>

{#if copy}
  <!-- One screen and one strip, same markup. tech.md 6.16. -->
  <div class="reach" class:compact>
    {#if !compact}
      <!-- Peekle's own sign, grey: grey is exactly what it means here. -->
      <svg
        class="sign"
        viewBox="0 0 {SIGN_BOX.width} {SIGN_BOX.height}"
        width="36"
        height="31"
        aria-hidden="true"
      >
        {#each SIGN_STROKES as stroke (stroke)}
          <path
            d={stroke}
            stroke="currentColor"
            stroke-width={SIGN_WEIGHT}
            stroke-linecap="round"
            fill="none"
          />
        {/each}
      </svg>
    {/if}

    <h2>{copy.title}</h2>
    <p>{copy.line}</p>

    <div class="row">
      <Button label={copy.action} variant="connect" wide {busy} onclick={() => onretry?.()} />
    </div>
  </div>
{/if}

<style>
  .reach {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 340px;
  }

  .reach.compact {
    align-items: stretch;
    text-align: left;
    max-width: none;
    width: 100%;
  }

  .compact h2 {
    font-size: 13px;
  }

  .compact p {
    margin-top: 3px;
    font-size: 11px;
    line-height: 1.4;
  }

  .compact .row {
    margin-top: 8px;
  }

  .compact .row :global(button) {
    font-size: 12px;
    padding: 8px 14px;
  }

  .sign {
    color: var(--text-dim);
    opacity: 0.5;
    margin-bottom: 20px;
  }

  h2 {
    margin: 0;
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text);
  }

  p {
    margin: 7px 0 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-dim);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 22px;
    width: 100%;
  }

  .row :global(button) {
    font-size: 13px;
    padding: 10px 22px;
    border-radius: 9px;
  }
</style>
