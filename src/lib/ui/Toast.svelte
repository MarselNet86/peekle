<script lang="ts">
  /**
   * Content of the pill: what happened, in the shape the system uses for the
   * same job. Who it was on the first line, what was said under it, and how
   * long it took at the end of that line. tech.md 6.2 and 9.
   *
   * The run-on line it replaced -- `peekle · Дошло: ответ...` -- put the name
   * and the words in one 13px string and clipped whichever ran out of room
   * first, which was always the words. Two lines cost nothing here: the pill
   * is drawn for this and grows to fit it.
   */
  import { elapsedLabel } from '$lib/logic/work';
  import type { ToastTone } from '$lib/types/generated/ToastTone';

  let {
    text,
    detail = null,
    tookMs = null,
    tone = 'Neutral',
    badge,
  }: {
    text: string;
    /** The quiet line under the first one. Without it the pill is one line. */
    detail?: string | null;
    /** How long the turn took, for a pill reporting one. tech.md 6.2. */
    tookMs?: number | null;
    tone?: ToastTone;
    badge?: number | null;
  } = $props();

  const took = $derived(
    tookMs !== null && Number.isFinite(tookMs) && tookMs > 0 ? elapsedLabel(tookMs) : null,
  );
</script>

<!-- Content of the pill only. The black fill, the corners and the movement
     belong to Shape. tech.md 9. -->
<div class="band" data-tone={tone} class:deep={detail !== null}>
  <span class="mark"></span>
  <span class="what">
    <span class="text">{text}</span>
    {#if detail !== null}
      <span class="under">
        <span class="detail">{detail}</span>
        {#if took}
          <span class="took">{took}</span>
        {/if}
      </span>
    {/if}
  </span>
  {#if badge != null}
    <span class="badge">{badge}</span>
  {/if}
</div>

<style>
  .band {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding: 0 18px;
    box-sizing: border-box;
    color: var(--text);
  }

  .mark {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  .band[data-tone='On'] .mark {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }
  .band[data-tone='Off'] .mark {
    background: var(--text-dim);
  }
  .band[data-tone='Warn'] .mark {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
  }
  .band[data-tone='Neutral'] .mark {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }

  .text {
    font-size: 13px;
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Whoever it was, said louder than what they said: with two chats running,
     which one finished is the thing the eye needs first. The same weight the
     permission panel puts on its own title. tech.md 6.7. */
  .band.deep .text {
    font-size: 15px;
    font-weight: 600;
  }

  .under {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }

  .detail {
    min-width: 0;
    font-size: 12px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* How long they were away. Never dropped when the words are clipped: it is
     the shortest thing on the line and the only number on it. */
  .took {
    flex: none;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    opacity: 0.7;
  }

  .badge {
    flex: none;
    min-width: 18px;
    padding: 1px 6px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
</style>
