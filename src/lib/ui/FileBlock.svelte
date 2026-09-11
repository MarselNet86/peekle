<script lang="ts">
  /**
   * A file as it stands inside a message: a document sign and the file's own
   * name. tech.md 6.25 and 9.
   *
   * Not a button, unlike the screenshot beside it. A shot has a picture to
   * open and this has nothing: the island does not open files with the system
   * and should not learn to. The whole path is one hover away, which is where
   * `ShotBlock` keeps it too -- a path is far too long to read in a bubble,
   * and the name is the half that says which file this is.
   */
  import { fileName } from '$lib/logic/files';

  let { path }: { path: string } = $props();

  const name = $derived(fileName(path));
</script>

<span class="file-block" title={path}>
  <svg viewBox="0 0 10 12" width="10" height="12" aria-hidden="true">
    <path
      d="M1 1.6a1 1 0 011-1h3.4L9 4.2v6.2a1 1 0 01-1 1H2a1 1 0 01-1-1z"
      fill="none"
      stroke="currentColor"
      stroke-width="1"
    />
    <path d="M5.4 0.8v3.2H8.8" fill="none" stroke="currentColor" stroke-width="1" />
  </svg>
  <span class="name">{name}</span>
</span>

<style>
  .file-block {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    max-width: 100%;
    padding: 5px 9px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    /* Dark and nearly opaque, for the reason `ShotBlock` is: the same block
       stands on the green of a reply and on the dark of an answer, and a
       lightening comes out pale green on the first. */
    background: rgba(12, 12, 14, 0.92);
    color: var(--text);
    font-size: 11px;
    line-height: 1;
  }

  svg {
    flex: none;
    opacity: 0.7;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
