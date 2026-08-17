<script lang="ts">
  let { visible = false }: { visible?: boolean } = $props();
</script>

<div class="hint" class:visible aria-hidden="true">
  <svg viewBox="0 0 12 8" width="12" height="8">
    <path d="M1 1l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1.5" />
  </svg>
</div>

<style>
  .hint {
    display: flex;
    justify-content: center;
    color: var(--text-dim);
    opacity: 0;
    transition: opacity 120ms ease;
    pointer-events: none;
  }

  .visible {
    opacity: 1;
  }
</style>
