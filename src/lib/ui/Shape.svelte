<script lang="ts">
  import { Spring } from 'svelte/motion';
  import { untrack, type Snippet } from 'svelte';

  import {
    FLOAT_HEAD,
    shapeBounds,
    shapeInset,
    type Notch,
    type ShapeBounds,
  } from '$lib/logic/shape';
  import type { IslandView } from '$lib/types/generated/IslandView';

  let {
    view,
    notch,
    badge = false,
    asking = false,
    deep = false,
    rest,
    children,
  }: {
    view: IslandView;
    notch: Notch;
    /** The resting shape stands open for the usage number. tech.md 6.18. */
    badge?: boolean;
    /** A question is standing in the dialogue, so the shape takes the whole
     * window until it is answered. tech.md 6.14. */
    asking?: boolean;
    /** The pill carries a second line, so it stands as tall as the panel that
     * carries two. tech.md 6.2. */
    deep?: boolean;
    rest?: Snippet;
    children?: Snippet;
  } = $props();

  /** Content follows the shape rather than arriving with it. tech.md 6.10. */
  const CONTENT_DELAY_MS = 60;

  const target = $derived(shapeBounds(view, notch, badge, asking, deep));
  const collapsed = $derived(view === 'Collapsed');
  const hasNotch = $derived(notch.height > 0);
  // Off the edge without a notch, flush with it under one. tech.md 6.7.
  const inset = $derived(shapeInset(notch));

  /**
   * How long the shape may spend on its way to a new size before it is simply
   * put there.
   *
   * The spring settles well inside this, so on a webview that keeps handing
   * out animation frames nothing below ever fires. What it catches is the one
   * that stops: on Windows the frame loop the spring rides goes quiet once the
   * page has loaded, and the box stayed at the resting capsule for good. The
   * view switched, the content swapped underneath it, Rust was told the island
   * was open -- and all the user saw was the same small pill, because the only
   * thing that never moved was its size. A shape that arrives without the
   * spring is worse than one that springs; a shape that never arrives is not a
   * shape. tech.md 6.7, 6.10 and 6.27.
   */
  const SETTLE_MS = 500;

  // One spring for the whole product, identical opening and closing, so growing
  // and collapsing read as one body rather than two effects. tech.md 6.10.
  const bounds = new Spring(
    untrack(() => ({ width: target.width, height: target.height, radius: target.radius })),
    { stiffness: 0.15, damping: 0.8 },
  );

  /** The size the spring never reached, or null while it is still the spring's
   * to reach. Drawn instead of `bounds.current` so the box lands on its target
   * whatever the frame loop is doing. */
  let stalled = $state<ShapeBounds | null>(null);
  const box = $derived(stalled ?? bounds.current);

  let contentShown = $state(false);

  $effect(() => {
    const next = { width: target.width, height: target.height, radius: target.radius };
    stalled = null;
    bounds.target = next;

    // The spring is read in the timer rather than in the body: reading it here
    // would make every frame of the animation restart the very timer whose job
    // is to survive an animation that never runs.
    const settle = setTimeout(() => {
      if (!arrived(bounds.current, next)) stalled = next;
    }, SETTLE_MS);
    return () => clearTimeout(settle);
  });

  /** Within a pixel on every side is arrived: a spring stops at its target, it
   * does not land on it exactly. */
  function arrived(now: ShapeBounds, wanted: ShapeBounds): boolean {
    return (
      Math.abs(now.width - wanted.width) < 1 &&
      Math.abs(now.height - wanted.height) < 1 &&
      Math.abs(now.radius - wanted.radius) < 1
    );
  }

  $effect(() => {
    if (collapsed) {
      contentShown = false;
      return;
    }
    const timer = setTimeout(() => {
      contentShown = true;
    }, CONTENT_DELAY_MS);
    return () => clearTimeout(timer);
  });
</script>

<!-- The window stays 720x560 forever. Only this shape moves. tech.md 6.7. -->
<div
  class="shape"
  class:collapsed
  data-view={typeof view === 'string' ? view : 'Session'}
  style:width="{box.width}px"
  style:height="{box.height}px"
  style:margin-top="{inset}px"
  style:border-radius={hasNotch ? `0 0 ${box.radius}px ${box.radius}px` : `${box.radius}px`}
>
  {#if collapsed && rest}
    <div class="rest">{@render rest()}</div>
  {/if}
  {#if children}
    <!-- The content starts under the cutout, and the band it steps over is
         handed on: a view with something to put beside the notch lifts its own
         top row into it. Both are zero on a display with no notch, so the same
         markup lands where it always did. tech.md 6.7. -->
    <div
      class="content"
      class:shown={contentShown}
      style:padding-top="{notch.height}px"
      style:--notch-h="{notch.height}px"
      style:--notch-w="{hasNotch ? notch.width : 0}px"
      style:--band-top="{hasNotch ? 0 : FLOAT_HEAD}px"
    >
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .shape {
    /* Exactly black and fully opaque. Any transparency or blur here gives away
       that this is a window on top of the system. tech.md 9 and 6.10. */
    background: var(--notch);
    position: relative;
    /* Centred; the top margin is the inset of 6.7, set per notch above. */
    margin: 0 auto;
    overflow: hidden;
    color: var(--text);
    box-sizing: border-box;
  }

  /* Collapsed is exactly the notch: the bezel hides the cutout itself, so what
     shows is the overhang on either side of it, and nothing hangs below.
     tech.md 6.7. */
  .shape.collapsed {
    pointer-events: none;
  }

  /* The one part of a collapsed island that takes a click. Rust hands the
     mouse over only while the pointer is inside it. tech.md 6.7. */
  .rest {
    position: absolute;
    inset: 0;
    pointer-events: auto;
  }

  .content {
    height: 100%;
    box-sizing: border-box;
    opacity: 0;
    transform: translateY(-4px);
    /* Opacity and offset only. Never filter or backdrop-filter: those repaint
       the whole area under the window on every frame. tech.md 6.10. */
    transition:
      opacity 140ms ease-out,
      transform 140ms ease-out;
  }

  .content.shown {
    opacity: 1;
    transform: none;
  }
</style>
