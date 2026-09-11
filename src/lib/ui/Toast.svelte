<script lang="ts">
  /**
   * Content of the pill: what happened, laid out the way the permission panel
   * lays out its own two lines. tech.md 6.2, 6.7 and 9.
   *
   * The two are the same object seen twice -- a band with a sign at its head,
   * two lines of words in the middle and one thing at its tail -- so they are
   * drawn the same. The difference is what the tail is for: the panel is
   * answered and carries buttons, the pill is only read and carries the one
   * number a finished turn has. Until v80.19 this one had a bare dot at its
   * head and nothing at its tail at all, so three quarters of a 420 pixel
   * band was empty black beside a sentence.
   *
   * The run-on line it replaced -- `peekle · Дошло: ответ...` -- put the name
   * and the words in one 13px string and clipped whichever ran out of room
   * first, which was always the words. Two lines cost nothing here: the pill
   * is drawn for this and grows to fit it.
   */
  import { elapsedLabel } from '$lib/logic/work';
  import { SIGN_BOX, SIGN_STROKES, SIGN_WEIGHT } from '$lib/logic/sign';
  import type { ToastTone } from '$lib/types/generated/ToastTone';

  let {
    text,
    detail = null,
    tookMs = null,
    tone = 'Neutral',
    badge,
    ttlMs = null,
  }: {
    text: string;
    /** The quiet line under the first one. Without it the pill is one line. */
    detail?: string | null;
    /** How long the turn took, for a pill reporting one. tech.md 6.2. */
    tookMs?: number | null;
    tone?: ToastTone;
    badge?: number | null;
    /** How long this pill stands, drawn as the hairline underneath it. The
     * panel draws its own twenty seconds the same way, and for the same
     * reason: a bar that leaks is read without being read. tech.md 6.7. */
    ttlMs?: number | null;
  } = $props();

  const took = $derived(
    tookMs !== null && Number.isFinite(tookMs) && tookMs > 0 ? elapsedLabel(tookMs) : null,
  );
  const secs = $derived(ttlMs !== null && ttlMs > 0 ? ttlMs / 1000 : null);
</script>

<!-- Content of the pill only. The black fill, the corners and the movement
     belong to Shape. tech.md 9. -->
<div class="band" data-tone={tone} class:deep={detail !== null}>
  <!-- The head of the band is the app, the way a notification on this system
       wears the icon of whatever raised it: every pill in the island is
       Peekle speaking, and the tone colours the sign rather than replacing
       it. A bare dot said the same thing in a language nobody reads.
       tech.md 9. -->
  <span class="mark" aria-hidden="true">
    <svg viewBox="0 0 {SIGN_BOX.width} {SIGN_BOX.height}" width="15" height="11">
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
  </span>

  <span class="what">
    <span class="text">{text}</span>
    {#if detail !== null}
      <span class="detail">{detail}</span>
    {/if}
  </span>

  <!-- The tail. A finished turn has exactly one number -- how long it was
       away -- and it stands where the panel stands its buttons, because that
       is where the eye goes after the words. tech.md 6.2. -->
  {#if took || badge != null}
    <span class="trail">
      {#if took}
        <span class="took">{took}</span>
      {/if}
      {#if badge != null}
        <span class="badge">{badge}</span>
      {/if}
    </span>
  {/if}

  {#if secs}
    <span class="leak" style:--secs="{secs}s"></span>
  {/if}
</div>

<style>
  .band {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 100%;
    padding: 0 16px 0 14px;
    box-sizing: border-box;
    color: var(--text);
  }

  /* A round badge, the size of an icon in a notification rather than a status
     dot: it is the head of the band, and the two lines beside it hang off it.
     tech.md 9. */
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    color: var(--accent);
  }

  /* The tone colours the strokes and never the disc. One ground for every
     pill keeps the head of the band the same object each time, which is what
     makes it read as an icon rather than as a status light -- and it keeps
     this off `color-mix`, which the oldest WebKit this product supports does
     not have. tech.md 3 and 9. */
  .band[data-tone='Off'] .mark {
    color: var(--text-dim);
  }

  .band[data-tone='Warn'] .mark {
    color: var(--warn);
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 2px;
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
    line-height: 1.2;
  }

  .detail {
    min-width: 0;
    font-size: 12px;
    line-height: 1.3;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .trail {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }

  /* How long they were away, drawn as a thing rather than as a footnote to
     the words: it is the one number a finished turn has, and it was the last
     thing on a line that was already being clipped. */
  .took {
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .badge {
    min-width: 18px;
    padding: 3px 7px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--notch);
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    text-align: center;
  }

  /* The pill's own clock, drawn the way the panel draws its twenty seconds.
     Transform only: a width animation relays out the shape on every frame.
     tech.md 6.10 and 6.7. */
  .leak {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    background: rgba(255, 255, 255, 0.22);
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
