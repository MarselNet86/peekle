<script lang="ts">
  /**
   * The island run down to one line while files are picked. tech.md 6.25 and 9.
   *
   * The system file dialog is an ordinary window of this app and the island is
   * a panel above every one of them, so the dialog opens behind it and the top
   * of it cannot even be pressed. The strip is the island getting out of the
   * way without losing what the person is in the middle of: it says how many
   * files are already on the message, and it says how to come back.
   */
  import { CHAT } from '$lib/i18n/chat';
  import { copy } from '$lib/i18n/index.svelte';
  import Button from './Button.svelte';

  let {
    count,
    picking = false,
    folder = false,
    onexpand,
  }: {
    /** The dialog under the strip chooses the folder of a new chat, not
     * files: nothing is counted, and the line says what is being chosen.
     * tech.md 6.23, v87.12. */
    folder?: boolean;
    /** How many files are attached to the message underneath. */
    count: number;
    /** The dialog is standing right now. Nothing is attached yet on the first
     * pass, and a strip that says "0 files" says nothing at all. */
    picking?: boolean;
    onexpand?: () => void;
  } = $props();

  const t = $derived(copy(CHAT));
  const said = $derived(
    folder ? t.pickingFolder : count > 0 ? t.filesAttached(count) : t.pickingFiles,
  );
</script>

<div class="strip">
  <!-- A page with its corner turned: the same sign the chip and the block in
       the feed wear, so one thing is drawn one way. tech.md 9. -->
  <svg viewBox="0 0 10 12" width="11" height="13" aria-hidden="true">
    <path
      d="M1 1.6a1 1 0 011-1h3.4L9 4.2v6.2a1 1 0 01-1 1H2a1 1 0 01-1-1z"
      fill="none"
      stroke="currentColor"
      stroke-width="1"
    />
    <path d="M5.4 0.8v3.2H8.8" fill="none" stroke="currentColor" stroke-width="1" />
  </svg>
  <span class="said" class:waiting={picking && (folder || count === 0)}>{said}</span>
  <!-- The way back, and it carries its key: the pointer is in the dialog, not
       here, so the combination is the one that will actually be used.
       tech.md 6.25. -->
  <Button label={t.expandIsland} variant="muted" shortcut="⌘1" onclick={() => onexpand?.()} />
</div>

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 100%;
    box-sizing: border-box;
    padding: 0 10px 0 16px;
    color: var(--text);
  }

  svg {
    flex: none;
    opacity: 0.7;
  }

  .said {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }

  /* Nothing picked yet: the line is a state, not a count, and it reads as one.
     tech.md 6.25. */
  .said.waiting {
    color: var(--text-dim);
  }
</style>
