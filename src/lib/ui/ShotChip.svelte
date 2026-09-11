<script lang="ts">
  // The picture and not its name: the file is a ulid, and which of two shots
  // this one is cannot be read off `01M0ZKJ7...`. tech.md 6.13.
  let {
    name,
    src = '',
    kind = 'shot',
    onopen,
    onremove,
  }: {
    name: string;
    src?: string;
    /** A file attached from disk has no thumbnail and never will: the asset
     * scope is the shots cache alone, and widening it to the whole disk for a
     * forty pixel preview hands the webview every file the user owns. So it
     * wears a document sign and its own name. tech.md 6.25. */
    kind?: 'shot' | 'file';
    onopen?: () => void;
    onremove?: () => void;
  } = $props();

  // No src outside the app shell, and a cache the user may empty at any
  // moment. Either way the attachment stays visible and stays removable.
  let broken = $state(false);
  const thumb = $derived(kind === 'shot' && src !== '' && !broken);
</script>

<span class="chip" class:thumb>
  {#if thumb}
    <!-- Forty pixels say which shot this is, not what is on it, and "is that
         the one I meant" is answered by the picture. tech.md 6.13. -->
    <button
      type="button"
      class="open"
      aria-label="Open {name}"
      onmousedown={(event) => event.preventDefault()}
      onclick={() => onopen?.()}
    >
      <img {src} alt={name} title={name} onerror={() => (broken = true)} />
    </button>
  {:else if kind === 'file'}
    <!-- A page with its corner turned: what a file is, at eleven pixels. -->
    <svg viewBox="0 0 10 12" width="10" height="12" aria-hidden="true">
      <path
        d="M1 1.6a1 1 0 011-1h3.4L9 4.2v6.2a1 1 0 01-1 1H2a1 1 0 01-1-1z"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
      />
      <path d="M5.4 0.8v3.2H8.8" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="name" title={name}>{name}</span>
  {:else}
    <svg viewBox="0 0 12 10" width="12" height="10" aria-hidden="true">
      <rect
        x="0.6"
        y="0.6"
        width="10.8"
        height="8.8"
        rx="1.6"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
      />
      <path d="M1.4 7.4l2.6-2.6 2 2 1.8-1.6 2.8 2.6" fill="none" stroke="currentColor" />
    </svg>
    <span class="name">{name}</span>
  {/if}
  <!-- mousedown is swallowed so taking an attachment back does not pull the
       caret out of the field beside it. tech.md 6.7. -->
  <button
    type="button"
    aria-label="Remove {name}"
    onmousedown={(event) => event.preventDefault()}
    onclick={() => onremove?.()}
  >
    <svg viewBox="0 0 8 8" width="8" height="8" aria-hidden="true">
      <path d="M1 1l6 6M7 1l-6 6" fill="none" stroke="currentColor" stroke-width="1.4" />
    </svg>
  </button>
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 200px;
    padding: 3px 6px 3px 8px;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--bubble);
    color: var(--text-dim);
    font-size: 11px;
  }

  /* A tile once there is a picture, and the cross rides its corner: a pill
     around an image is a frame around a frame. */
  .chip.thumb {
    position: relative;
    gap: 0;
    padding: 0;
    border-radius: 8px;
    overflow: visible;
  }

  img {
    display: block;
    width: 56px;
    height: 40px;
    border-radius: 7px;
    object-fit: cover;
    /* The shot is of a screen, so its own edges are the picture. */
    background: var(--bubble);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Sized by the picture inside it, not by the cross rule below. */
  .open {
    display: block;
    width: auto;
    height: auto;
    padding: 0;
    border: none;
    border-radius: 7px;
    background: transparent;
    cursor: pointer;
  }

  button {
    display: grid;
    place-items: center;
    flex: none;
    width: 14px;
    height: 14px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  button:hover {
    color: var(--text);
  }

  .chip.thumb button:not(.open) {
    position: absolute;
    top: -5px;
    right: -5px;
    border: 1px solid var(--hairline);
    background: var(--notch);
    color: var(--text);
  }
</style>
