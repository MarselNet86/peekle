<script lang="ts">
  import { Power } from '@lucide/svelte';

  import { copy } from '$lib/i18n/index.svelte';
  import { QUIT } from '$lib/i18n/quit';
  import Button from './Button.svelte';

  let {
    busy = false,
    onyes,
    onno,
  }: {
    /** Yes was pressed and the island is folding on its way out. */
    busy?: boolean;
    onyes?: () => void;
    onno?: () => void;
  } = $props();

  const t = $derived(copy(QUIT));
  const id = $props.id();

  // Escape is no. The panel does not hold the keyboard until it is clicked,
  // so this is a second way out rather than the only one. tech.md 6.29.
  function keydown(event: KeyboardEvent) {
    if (busy || event.key !== 'Escape') return;
    event.preventDefault();
    onno?.();
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- One line and two buttons, the island open just enough to ask. Yes is the
     dark one and No the white one: the safe answer is the one the eye lands
     on. tech.md 6.29. -->
<div
  class="quit"
  class:leaving={busy}
  role="alertdialog"
  aria-labelledby="{id}-title"
  aria-describedby="{id}-line"
>
  <span class="sign" aria-hidden="true"><Power size={15} strokeWidth={2} /></span>

  <div class="what">
    <span class="title" id="{id}-title">{t.title}</span>
    <span class="line" id="{id}-line">{t.line}</span>
  </div>

  <div class="answers">
    <span class="answer" style="--order: 0">
      <Button label={t.yes} variant="muted" disabled={busy} onclick={() => onyes?.()} />
    </span>
    <span class="answer" style="--order: 1">
      <Button label={t.no} variant="prominent" disabled={busy} onclick={() => onno?.()} />
    </span>
  </div>
</div>

<style>
  .quit {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 100%;
    padding: 0 16px 0 16px;
    box-sizing: border-box;
    /* Up from a few pixels below, on the curve every entrance rides, and
       back down again on the way out. Opacity and transform only. tech.md
       6.10. */
    animation: arrive 260ms cubic-bezier(0.22, 1, 0.36, 1) both;
    transition:
      opacity 200ms ease,
      transform 200ms ease;
  }

  .quit.leaving {
    opacity: 0;
    transform: scale(0.97);
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

  .quit:focus,
  .quit:focus-visible {
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

  .answers {
    display: flex;
    gap: 8px;
    flex: none;
  }

  /* One after the other, so the two answers arrive as a choice rather than
     as a block. */
  .answer {
    display: inline-flex;
    animation: arrive 300ms cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: calc(80ms + var(--order) * 60ms);
  }

  @media (prefers-reduced-motion: reduce) {
    .quit,
    .sign,
    .answer {
      animation: none;
    }

    .quit {
      transition: none;
    }
  }
</style>
