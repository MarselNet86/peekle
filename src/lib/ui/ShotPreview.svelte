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
  aria-label="Close the shot"
  onclick={() => onclose?.()}
  onkeydown={keydown}
>
  <img {src} alt={name} />
  <!-- The whole layer closes it, but a picture filling the island does not
       look pressable and a cross does. -->
  <button
    type="button"
    aria-label="Close {name}"
    onmousedown={(event) => event.preventDefault()}
    onclick={(event) => {
      // The layer under it closes on a click too, and one press is one close.
      event.stopPropagation();
      onclose?.();
    }}
  >
    <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true">
      <path d="M1 1l8 8M9 1l-8 8" fill="none" stroke="currentColor" stroke-width="1.5" />
    </svg>
  </button>
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

  button {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--hairline);
    border-radius: 50%;
    background: var(--notch);
    color: var(--text-dim);
    cursor: pointer;
  }

  button:hover {
    color: var(--text);
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
