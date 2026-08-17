<script lang="ts">
  import type { Snippet } from 'svelte';
  import '../app.css';

  let { children }: { children: Snippet } = $props();
</script>

{@render children()}

<style>
  /* The overlay sits on top of whatever the user is watching. Any opaque
     ground here would black out the screen behind the window. */
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent;
    color: var(--text);
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif;
    overflow: hidden;
  }

  /* A visible scrollbar reads as a browser chrome artifact on a floating panel. */
  :global(::-webkit-scrollbar) {
    width: 0;
    height: 0;
  }
</style>
