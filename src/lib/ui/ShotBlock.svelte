<script lang="ts">
  /**
   * A screenshot as it stands inside a message: a small tile, the word for
   * what it is, and how big it is. Pressing it opens the whole picture.
   * tech.md 6.13 and 9.
   *
   * It used to be the picture itself, 220 by 110 in the bubble. That is a
   * strange size for both jobs: too small to read anything on a screenshot of
   * a screen, and big enough to push the words that came with it off the
   * island. So the message carries a reference -- the tile says which shot,
   * the numbers say what will open -- and the picture opens in full, which is
   * where it was always going to be read.
   */
  import { shotSize } from '$lib/logic/shots';

  let {
    name,
    src = '',
    onopen,
    onbroken,
  }: {
    /** The file, for the pointer. It is a ulid and says nothing about the
     * picture, which is why it is not the label. tech.md 6.13. */
    name: string;
    src?: string;
    onopen?: () => void;
    /** The picture would not load. The message says so by putting the path
     * back: a reply has to show what actually went to the agent. */
    onbroken?: () => void;
  } = $props();

  let size = $state<string | null>(null);

  function measured(event: Event) {
    const image = event.currentTarget as HTMLImageElement;
    size = shotSize(image.naturalWidth, image.naturalHeight);
  }
</script>

<button
  type="button"
  class="shot-block"
  title={name}
  aria-label="Open the screenshot{size ? `, ${size}` : ''}"
  onclick={() => onopen?.()}
>
  <img {src} alt="" onload={measured} onerror={() => onbroken?.()} />
  <span class="what">Screenshot</span>
  <!-- The one thing about a picture that can be said in a line, and the one
       thing that tells two of them apart at this size. -->
  {#if size}
    <span class="size">{size}</span>
  {/if}
</button>

<style>
  .shot-block {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    max-width: 100%;
    padding: 4px 9px 4px 4px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    /* Dark, and nearly opaque, because the ground under it is not one ground:
       the same block stands on the green of a reply and on the dark of an
       answer. `--bubble` is a lightening, so on green it came out pale green
       and read as part of the bubble rather than as a thing in it. */
    background: rgba(12, 12, 14, 0.92);
    color: var(--text);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
  }

  .shot-block:hover {
    border-color: var(--text-dim);
  }

  /* Enough to recognise the shot by, and no more: what is on it is read in
     the full view, one press away. */
  img {
    display: block;
    flex: none;
    width: 22px;
    height: 18px;
    border-radius: 5px;
    object-fit: cover;
    /* A screenshot of this product is mostly black, so on a dark block the
       tile would be a hole in it. The outline is what says a picture is
       there at all. */
    border: 1px solid var(--hairline);
    background: var(--surface);
  }

  .what {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Quieter than the word beside it, but out of the message's own colour
     rather than a token: this block stands on the green bubble as well as the
     dark one, and `--text-dim` on `--brand` is barely there. */
  .size {
    flex: none;
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
  }
</style>
