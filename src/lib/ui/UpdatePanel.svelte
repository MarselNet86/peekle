<script lang="ts">
  import { ArrowDownToLine } from '@lucide/svelte';

  import { copy } from '$lib/i18n/index.svelte';
  import { UPDATE } from '$lib/i18n/update';
  import { readableSize } from '$lib/logic/update';
  import Button from './Button.svelte';

  let {
    version,
    size = 0,
    brew = false,
    busy = false,
    oninstall,
    onlater,
    onnotes,
  }: {
    /** The tag without its v, as it is shown: "0.1.2". */
    version: string;
    /** Bytes of the downloaded file. Zero when the release named no size. */
    size?: number;
    /** Homebrew owns this copy, so the answer is a command, not a file. */
    brew?: boolean;
    /** Install was pressed and the island is folding on its way out. */
    busy?: boolean;
    oninstall?: () => void;
    onlater?: () => void;
    onnotes?: () => void;
  } = $props();

  const t = $derived(copy(UPDATE));
  const id = $props.id();

  const line = $derived.by(() => {
    if (brew) return t.brew;
    const readable = readableSize(size);
    return readable ? t.ready(readable) : t.readyNoSize;
  });

  // Escape is later. The panel does not hold the keyboard until it is clicked,
  // so this is a second way out rather than the only one. tech.md 6.30.
  function keydown(event: KeyboardEvent) {
    if (busy || event.key !== 'Escape') return;
    event.preventDefault();
    onlater?.();
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- One line and two buttons, the island open just enough to ask. Install is
     the white one here and Later the dark one: this is the one panel where the
     eye should land on the action, because the file is already on disk and
     putting it off costs more than taking it. tech.md 6.30. -->
<div
  class="update"
  class:leaving={busy}
  role="alertdialog"
  aria-labelledby="{id}-title"
  aria-describedby="{id}-line"
>
  <span class="sign" aria-hidden="true"><ArrowDownToLine size={15} strokeWidth={2} /></span>

  <div class="what">
    <!-- The title doubles as the way to the release page: whoever would rather
         read before installing should not have to look for the link. -->
    <button
      class="title"
      id="{id}-title"
      title={t.notes}
      disabled={busy}
      onclick={() => onnotes?.()}
    >
      {t.title(version)}
    </button>
    <span class="line" id="{id}-line">{line}</span>
  </div>

  <div class="answers">
    <span class="answer" style="--order: 0">
      <Button
        label={brew ? t.brewInstall : t.install}
        variant="prominent"
        disabled={busy}
        onclick={() => oninstall?.()}
      />
    </span>
    <span class="answer" style="--order: 1">
      <Button label={t.later} variant="muted" disabled={busy} onclick={() => onlater?.()} />
    </span>
  </div>
</div>

<style>
  .update {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 100%;
    padding: 0 16px;
    box-sizing: border-box;
    /* Up from a few pixels below, on the curve every entrance rides, and back
       down again on the way out. Opacity and transform only. tech.md 6.10. */
    animation: arrive 260ms cubic-bezier(0.22, 1, 0.36, 1) both;
    transition:
      opacity 200ms ease,
      transform 200ms ease;
  }

  .update.leaving {
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

  .update:focus,
  .update:focus-visible {
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
      transform: scale(0.6) translateY(-6px);
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
    align-items: flex-start;
  }

  .title {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    max-width: 100%;
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    text-align: left;
  }

  .title:hover:not(:disabled),
  .title:focus-visible {
    text-decoration: underline;
  }

  .title:disabled {
    cursor: default;
  }

  .line {
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .answers {
    display: flex;
    gap: 8px;
    flex: none;
  }

  /* One after the other, so the two answers arrive as a choice rather than as
     a block. */
  .answer {
    display: inline-flex;
    animation: arrive 300ms cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: calc(80ms + var(--order) * 60ms);
  }

  @media (prefers-reduced-motion: reduce) {
    .update,
    .sign,
    .answer {
      animation: none;
    }

    .update {
      transition: none;
    }
  }
</style>
