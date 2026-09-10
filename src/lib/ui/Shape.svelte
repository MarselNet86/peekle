<script lang="ts">
  import { Spring } from 'svelte/motion';
  import { untrack, type Snippet } from 'svelte';

  import { shapeBounds, type Notch } from '$lib/logic/shape';
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

  // One spring for the whole product, identical opening and closing, so growing
  // and collapsing read as one body rather than two effects. tech.md 6.10.
  const bounds = new Spring(
    untrack(() => ({ width: target.width, height: target.height, radius: target.radius })),
    { stiffness: 0.15, damping: 0.8 },
  );

  let contentShown = $state(false);

  $effect(() => {
    bounds.target = { width: target.width, height: target.height, radius: target.radius };
  });

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
  style:width="{bounds.current.width}px"
  style:height="{bounds.current.height}px"
  style:border-radius={hasNotch
    ? `0 0 ${bounds.current.radius}px ${bounds.current.radius}px`
    : `${bounds.current.radius}px`}
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
