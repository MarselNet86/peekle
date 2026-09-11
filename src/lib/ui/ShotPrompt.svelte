<script lang="ts">
  /**
   * The screenshot offer, laid out as the band every other pill is (6.2): a
   * sign at its head, two lines in the middle, one thing at its tail. The
   * thing at the tail is the key itself, drawn the size of a key, because
   * pressing it is the whole of what this pill is for -- and the line under
   * the project says so in words, for the person who is already reading.
   * tech.md 6.13 and 9.
   */
  import Kbd from '$lib/ui/Kbd.svelte';

  let {
    project,
    keys = ['↑'],
    left = 1,
    secs = 0,
    onopen,
  }: {
    project: string;
    keys?: string[];
    left?: number;
    secs?: number;
    /** Opens the chat the shot is offered to. The key attaches it; this is
     * for the person who wants to look at the chat first. tech.md 6.2. */
    onopen?: () => void;
  } = $props();

  function press(event: KeyboardEvent) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    onopen?.();
  }
</script>

<!-- Content of the pill only. The black fill, the corners and the movement
     belong to Shape. tech.md 9. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="band"
  class:pressable={onopen !== undefined}
  role={onopen ? 'button' : undefined}
  tabindex={onopen ? 0 : undefined}
  aria-label={onopen ? `Open ${project}` : undefined}
  onclick={onopen}
  onkeydown={onopen ? press : undefined}
>
  <span class="mark" aria-hidden="true">
    <svg viewBox="0 0 14 12" width="15" height="13">
      <rect
        x="0.75"
        y="2.75"
        width="12.5"
        height="8.5"
        rx="2"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
      />
      <circle cx="7" cy="7" r="2.4" fill="none" stroke="currentColor" stroke-width="1.2" />
      <path d="M4.6 2.5l1-1.5h2.8l1 1.5" fill="none" stroke="currentColor" stroke-width="1.2" />
    </svg>
  </span>

  <span class="what">
    <span class="text">Screenshot to {project}</span>
    <span class="under">
      <span class="how">Press the up arrow to attach it</span>
      <!-- The one question of the moment is whether there is time to reach
           the key, and only a number answers it. Zero is never shown: at zero
           there is no offer left to show it on. tech.md 6.13. -->
      {#if secs > 0}
        <span class="secs">{secs}s</span>
      {/if}
    </span>
  </span>

  <Kbd {keys} cap />

  <!-- The same deadline as a line, moved by a frame of the screen rather than
       by a timer, so it flows instead of stepping. tech.md 6.13. -->
  <span class="fuse" style="--left: {Math.min(1, Math.max(0, left))}"></span>
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

  .band.pressable {
    cursor: pointer;
  }

  /* The head of the band, the same disc the notice wears, with the sign of
     what this one is about. tech.md 6.2. */
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    color: var(--brand);
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .text {
    font-size: 15px;
    font-weight: 600;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .under {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }

  .how {
    min-width: 0;
    font-size: 12px;
    line-height: 1.3;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .secs {
    flex: none;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    opacity: 0.7;
  }

  .fuse {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    width: calc(var(--left) * 100%);
    background: rgba(255, 255, 255, 0.22);
  }
</style>
