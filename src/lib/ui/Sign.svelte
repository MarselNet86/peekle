<script lang="ts">
  import { SIGN_BOX, SIGN_STROKES, SIGN_WEIGHT } from '$lib/logic/sign';

  let {
    size = 56,
    label = 'Nothing said in this chat yet',
    caption = null,
  }: {
    /** How wide the sign is drawn. The height follows the box, because the
     * ratio of thickness to length is what the sign is recognised by. */
    size?: number;
    label?: string;
    /** A line under the strokes, or null for the sign on its own. The words
     * belong to whoever puts the sign somewhere, not to the sign. */
    caption?: string | null;
  } = $props();

  const height = $derived(Math.round((size * SIGN_BOX.height) / SIGN_BOX.width));
</script>

<!-- The product's sign, standing still in a place that has nothing to show.
     Still on purpose: movement in the island means the agent is working
     (6.12), and here nobody is. tech.md 9. -->
<span class="blank" role="img" aria-label={label}>
  <span class="sign">
    <svg viewBox="0 0 {SIGN_BOX.width} {SIGN_BOX.height}" width={size} {height} aria-hidden="true">
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
  <!-- Read after the sign, never instead of it: the strokes say whose window
       this is, and the line says what the window is waiting for. The label
       above carries both to a reader who hears the island. tech.md 9. -->
  {#if caption}
    <span class="caption">{caption}</span>
  {/if}
</span>

<style>
  .blank {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  /* Quiet enough to be the room's furniture rather than its content: the
     first thing said takes the sign's place, and a sign that shouted would
     have been read as something to do. Never a fill or a surface -- the
     island is exactly black, and this is two strokes on it. tech.md 9. */
  .sign {
    display: flex;
    color: var(--brand);
    opacity: 0.18;
  }

  /* Dimmer than anything the conversation itself puts on screen, and dimmer
     than the field's own placeholder: it is the last thing read in an empty
     room, not the first. tech.md 9. */
  .caption {
    font-size: 12px;
    letter-spacing: 0.2px;
    color: var(--text-dim);
    opacity: 0.55;
  }
</style>
