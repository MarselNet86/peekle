<script lang="ts">
  import type { RestStatus } from '$lib/logic/rest';

  let { status, onopen }: { status: RestStatus; onopen: () => void } = $props();

  const labels: Record<RestStatus, string> = {
    idle: 'Peekle is running. Open the session list',
    working: 'Claude is working. Open the session list',
    waiting: 'Claude is waiting on you. Open the session list',
  };
</script>

<!-- The whole resting strip is the target. A 14px band is small enough as it
     is without asking the user to hit a glyph inside it. tech.md 6.7. -->
<button class="mark" data-status={status} aria-label={labels[status]} onclick={onopen}>
  <svg viewBox="0 0 14 14" width="12" height="12" aria-hidden="true">
    <path
      d="M1 7c1.8-3 4-4.5 6-4.5S11.2 4 13 7c-1.8 3-4 4.5-6 4.5S2.8 10 1 7z"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
    />
    <circle cx="7" cy="7" r="1.9" fill="currentColor" />
  </svg>
</button>

<style>
  .mark {
    display: flex;
    align-items: flex-end;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 0 0 1px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    /* Colour only. The shape underneath belongs to Shape and never moves for
       a hover. tech.md 6.10. */
    transition: color 160ms ease-out;
  }

  .mark[data-status='working'] {
    color: var(--text);
  }

  /* Waiting is the one state that costs the user time, so it is the one state
     that moves. Opacity only: filter and backdrop-filter repaint everything
     under the window on every frame. tech.md 6.10. */
  .mark[data-status='waiting'] {
    color: var(--accent);
    animation: breathe 1600ms ease-in-out infinite;
  }

  .mark:hover {
    color: var(--text);
  }

  @keyframes breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.45;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mark[data-status='waiting'] {
      animation: none;
    }
  }
</style>
