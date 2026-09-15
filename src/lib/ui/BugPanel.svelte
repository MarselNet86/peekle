<script lang="ts">
  import { Bug } from '@lucide/svelte';

  import { copy } from '$lib/i18n/index.svelte';
  import { BUG } from '$lib/i18n/bug';
  import Button from './Button.svelte';

  let {
    busy = false,
    error = null,
    onwrite,
    oncancel,
  }: {
    /** Write was pressed and the issue page is being opened. */
    busy?: boolean;
    /** Why the issue page did not open, in words. Stands in the line under the
     * title, so the press that failed says so where the eye already is. */
    error?: string | null;
    onwrite?: () => void;
    oncancel?: () => void;
  } = $props();

  const t = $derived(copy(BUG));
  const id = $props.id();

  // Escape is cancel, as No is on the quit question. tech.md 6.22.
  function keydown(event: KeyboardEvent) {
    if (busy || event.key !== 'Escape') return;
    event.preventDefault();
    oncancel?.();
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- One line and two buttons, the island open just enough to ask. Write is
     the white one: the person pressed the bug button to report something, and
     the answer they came for is the one the eye lands on. tech.md 6.22. -->
<div class="bug" role="alertdialog" aria-labelledby="{id}-title" aria-describedby="{id}-line">
  <span class="sign" aria-hidden="true"><Bug size={15} strokeWidth={2} /></span>

  <div class="what">
    <span class="title" id="{id}-title">{t.title}</span>
    <span class="line" class:error={error !== null} id="{id}-line">{error ?? t.line}</span>
  </div>

  <div class="answers">
    <span class="answer" style="--order: 0">
      <Button
        label={t.write}
        variant="prominent"
        {busy}
        disabled={busy}
        onclick={() => onwrite?.()}
      />
    </span>
    <span class="answer" style="--order: 1">
      <Button label={t.cancel} variant="muted" disabled={busy} onclick={() => oncancel?.()} />
    </span>
  </div>
</div>

<style>
  .bug {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 100%;
    padding: 0 16px 0 16px;
    box-sizing: border-box;
    /* The entrance of the quit question, for the same kind of question.
       Opacity and transform only. tech.md 6.10. */
    animation: arrive 260ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .bug:focus,
  .bug:focus-visible {
    outline: none;
  }

  .sign {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
    animation: pop 460ms cubic-bezier(0.34, 1.56, 0.64, 1) 60ms both;
  }

  @keyframes pop {
    0% {
      opacity: 0;
      transform: scale(0.6) rotate(-40deg);
    }
    100% {
      opacity: 1;
      transform: none;
    }
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .line {
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .line.error {
    color: var(--danger, #ff6b6b);
  }

  .answers {
    display: flex;
    gap: 8px;
    flex: none;
  }

  .answer {
    display: inline-flex;
    animation: arrive 300ms cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: calc(80ms + var(--order) * 60ms);
  }

  @media (prefers-reduced-motion: reduce) {
    .bug,
    .sign,
    .answer {
      animation: none;
    }
  }
</style>
