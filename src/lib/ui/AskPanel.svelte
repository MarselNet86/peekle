<script lang="ts">
  import type { PromptRequest } from '$lib/types/generated/PromptRequest';
  import { ASK_SECS, secsLeft } from '$lib/features/permission/permission.svelte';
  import Button from './Button.svelte';

  let {
    request,
    onallow,
    ondeny,
    onopen,
  }: {
    request: PromptRequest;
    onallow?: () => void;
    ondeny?: () => void;
    /** Show me what I am agreeing to: the session this came from. tech.md 6.7. */
    onopen?: () => void;
  } = $props();

  let left = $state(ASK_SECS);

  // The panel stands for twenty seconds and says so. The hairline underneath
  // carries the same number as an animation, because a bar that leaks is read
  // without being read. tech.md 6.7.
  $effect(() => {
    const tick = () => (left = secsLeft(request.created_at, Date.now()));
    tick();
    const timer = setInterval(tick, 250);
    return () => clearInterval(timer);
  });

  /**
   * Digits and arrows answer without the mouse, the same two the row in the
   * feed takes. Watched on the window: the panel is non-activating and nothing
   * inside it holds focus until it is clicked. tech.md 6.7.
   */
  function keydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if (target && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT')) return;

    switch (event.key) {
      case '1':
      case 'ArrowLeft':
        event.preventDefault();
        ondeny?.();
        break;
      case '2':
      case 'ArrowRight':
        event.preventDefault();
        onallow?.();
        break;
    }
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- The whole body is the way into the session, and the buttons sit on top of
     it. A person who wants to read the command before answering should not
     have to find a link for it. tech.md 6.7. -->
<div
  class="ask"
  role="button"
  tabindex="-1"
  onclick={() => onopen?.()}
  onkeydown={() => {}}
  aria-label="Open the session this is asking about"
>
  <div class="what">
    <span class="title">{request.title}</span>
    <span class="under">
      {#if request.detail}
        <span class="detail">{request.detail}</span>
      {/if}
      <span class="clock" aria-hidden="true">{left}s</span>
    </span>
  </div>

  <!-- Stops the press from also opening the session underneath. -->
  <div class="answers" role="none" onclick={(event) => event.stopPropagation()}>
    <Button label="Deny" variant="muted" onclick={() => ondeny?.()} />
    <Button label="Allow" variant="prominent" onclick={() => onallow?.()} />
  </div>

  <span class="leak" style:--secs="{ASK_SECS}s"></span>
</div>

<style>
  .ask {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 100%;
    padding: 0 16px 0 18px;
    box-sizing: border-box;
    cursor: pointer;
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  .ask:focus,
  .ask:focus-visible {
    outline: none;
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }

  .title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* What the tool was actually handed. This is the thing being agreed to, so
     it is never summarised, only clipped. tech.md 6.7. */
  .detail {
    min-width: 0;
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .answers {
    display: flex;
    gap: 8px;
    flex: none;
  }

  .under {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }

  .clock {
    flex: none;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    opacity: 0.7;
  }

  /* The twenty seconds, drawn. Transform only: a width animation relays out
     the panel on every frame. tech.md 6.10. */
  .leak {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    /* Neutral, not green: green means Peekle in this product, and a clock is
       not the product saying anything. tech.md 9. */
    background: rgba(255, 255, 255, 0.3);
    transform-origin: left center;
    animation: leak var(--secs) linear forwards;
  }

  @keyframes leak {
    from {
      transform: scaleX(1);
    }
    to {
      transform: scaleX(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .leak {
      animation: none;
      transform: scaleX(0.02);
    }
  }
</style>
