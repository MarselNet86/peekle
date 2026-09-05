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
    /* Nothing here is selectable by default. The window is a transparent
       720x560 rectangle over somebody else's screen, and a drag that crosses
       it selects its markup: the highlight paints elements rather than text,
       so the invisible parts of the window go grey too and half the screen
       turns into a sheet the user reads as a bug. tech.md 9. */
    -webkit-user-select: none;
    user-select: none;
  }

  /* Where text is taken by hand, it stays selectable. */
  :global(input),
  :global(textarea) {
    -webkit-user-select: text;
    user-select: text;
  }

  /* A thumbnail dragged out of the island lands in whatever window is under
     it. Nothing here is a drag source. tech.md 6.13. */
  :global(img) {
    -webkit-user-drag: none;
  }

  /* A visible scrollbar reads as a browser chrome artifact on a floating panel. */
  :global(::-webkit-scrollbar) {
    width: 0;
    height: 0;
  }
</style>
