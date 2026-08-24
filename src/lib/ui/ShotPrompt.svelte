<script lang="ts">
  import Kbd from '$lib/ui/Kbd.svelte';

  let {
    project,
    keys = ['↑'],
    left = 1,
  }: { project: string; keys?: string[]; left?: number } = $props();
</script>

<!-- Content of the pill only. The black fill, the corners and the movement
     belong to Shape. tech.md 9. -->
<div class="band">
  <svg class="frame" viewBox="0 0 14 12" width="14" height="12" aria-hidden="true">
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
  <span class="text">Screenshot to {project}</span>
  <Kbd {keys} />
  <!-- The offer answers itself in five seconds, so it shows the five seconds
       going. A countdown in numbers would be read as an alarm. tech.md 6.13. -->
  <span class="fuse" style="--left: {Math.min(1, Math.max(0, left))}"></span>
</div>

<style>
  .band {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding: 0 18px;
    box-sizing: border-box;
    color: var(--text);
  }

  .frame {
    flex: none;
    color: var(--brand);
  }

  .text {
    flex: 1;
    font-size: 13px;
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .fuse {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 1.5px;
    width: calc(var(--left) * 100%);
    background: var(--brand);
    opacity: 0.7;
  }
</style>
