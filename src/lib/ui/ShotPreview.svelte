<script lang="ts">
  /**
   * One attachment, at the size it was taken. tech.md 6.13.
   *
   * A layer over the island's content and not a view of its own: opening a
   * picture is not a state Rust decides, and taking the view for it would put
   * the reply the user is writing behind a photo.
   */
  let { name, src, onclose }: { name: string; src: string; onclose?: () => void } = $props();

  function keydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    onclose?.();
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- The whole layer closes it: a picture opened by one click is closed by the
     next one, wherever it lands. -->
<div
  class="preview"
  role="button"
  tabindex="-1"
  aria-label="Close {name}"
  onclick={() => onclose?.()}
  onkeydown={keydown}
>
  <img {src} alt={name} />
</div>

<style>
  .preview {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: grid;
    place-items: center;
    padding: 14px;
    box-sizing: border-box;
    background: rgba(0, 0, 0, 0.72);
    cursor: zoom-out;
  }

  img {
    max-width: 100%;
    max-height: 100%;
    border-radius: 10px;
    /* The shot is of a screen and its own edges are the picture, so it gets a
       hairline instead of a frame. */
    border: 1px solid var(--hairline);
    object-fit: contain;
  }
</style>
