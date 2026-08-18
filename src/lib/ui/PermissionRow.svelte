<script lang="ts">
  import type { PromptRequest } from '$lib/types/generated/PromptRequest';
  import Button from './Button.svelte';

  let {
    request,
    onallow,
    ondeny,
  }: { request: PromptRequest; onallow?: () => void; ondeny?: () => void } = $props();

  /**
   * Digits and arrows answer without the mouse. The keys are watched on the
   * window rather than on a focused element: the panel is non-activating, so
   * nothing inside it holds focus until the user clicks. tech.md 6.7 and S4.
   */
  function keydown(event: KeyboardEvent) {
    // Never swallow a keystroke meant for a field the user is typing in.
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

<div class="permission">
  <div class="head">
    <span class="title">{request.title}</span>
    {#if request.detail}
      <span class="detail">{request.detail}</span>
    {/if}
  </div>

  <div class="actions">
    <Button label="Deny" onclick={() => ondeny?.()} />
    <Button label="Allow" variant="primary" onclick={() => onallow?.()} />
  </div>
</div>

<style>
  .permission {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .title {
    font-size: 14px;
    color: var(--text);
  }

  /* The tool input, verbatim and clipped. This is what the user is agreeing
     to, so it is never summarised. */
  .detail {
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
