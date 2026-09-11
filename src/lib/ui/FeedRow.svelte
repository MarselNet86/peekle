<script lang="ts">
  import { blocks } from '$lib/logic/markdown';
  import { FOLD_AT } from '$lib/logic/feed';
  import { fileLines } from '$lib/logic/files';
  import { shotLines, shotName } from '$lib/logic/shots';
  import FileBlock from './FileBlock.svelte';
  import ShotBlock from './ShotBlock.svelte';
  import type { Piece } from '$lib/logic/markdown';
  import type { FeedEntry } from '$lib/types/generated/FeedEntry';

  let {
    entry,
    shotSrc,
    onopenshot,
  }: {
    entry: FeedEntry;
    /** How a saved shot becomes something the webview can draw. Absent in the
     * showcase and wherever no shot can appear. tech.md 6.13. */
    shotSrc?: (path: string) => string;
    onopenshot?: (path: string) => void;
  } = $props();

  const spoken = $derived(entry.kind === 'User' || entry.kind === 'Assistant');
  // Not a message and not an object with a body: a line the conversation
  // records about itself. tech.md 6.15.
  const notice = $derived(entry.kind === 'Notice');
  // Collapsed like the terminal shows it, opened by a click. tech.md 6.12.
  let open = $state(false);
  // What the reply carries and what it says. A shot travels to the agent as a
  // path on its own line, which is right for the agent and useless to the
  // person who sent it: a ulid says neither which shot it is nor what is on
  // it. So the line becomes the picture again here. tech.md 6.13.
  const carried = $derived(spoken ? shotLines(entry.text) : { shots: [], said: entry.text });
  // Paths whose picture would not load. The line comes back for those: a
  // reply has to show what actually went to the agent.
  let broken = $state<string[]>([]);
  const shown = $derived(shotSrc ? carried.shots.filter((path) => !broken.includes(path)) : []);
  // And what it carried from disk. A file has no picture and nothing to open:
  // the block says which file, and the path it says it by is the line that
  // went to the agent. tech.md 6.25.
  //
  // A reply of one's own and never an answer. Only a person attaches; an agent
  // that puts a bare path on a line is naming a file it touched, which is a
  // word about a file and not a file, and drawing it as one would take the
  // path out of an answer that was written to carry it.
  const withFiles = $derived(
    entry.kind === 'User' ? fileLines(carried.said) : { files: [], said: carried.said },
  );
  // Whatever could not be drawn stays a line of the message, so nothing the
  // agent received disappears from the reply that sent it.
  const said = $derived(
    [...carried.shots.filter((path) => !shown.includes(path)), withFiles.said]
      .filter((line) => line !== '')
      .join('\n'),
  );
  // Parsed into segments and rendered as elements. Never `{@html}`: this text
  // comes out of an agent turn into a window over the whole screen.
  const parts = $derived(spoken ? blocks(said) : []);

  // Long messages of one's own are folded. What the person wrote they have
  // already read, and a page of it pushes the answer they are waiting for off
  // the island; an answer is never folded, because reading it is what the
  // island is for. tech.md 6.12.
  const foldable = $derived(entry.kind === 'User');
  let body = $state<HTMLElement | null>(null);
  let folds = $state(false);
  let unfolded = $state(false);

  /** The message itself is the control: pressing it opens and closes it.
   * Nothing is written under it -- a word saying `Show more` is one more thing
   * to read in a window where the whole point is reading something else, and
   * the fade already says there is more. tech.md 6.12. */
  function toggleFold() {
    // A press that ends a drag across the text is somebody copying, not
    // somebody opening. Messages are selectable on purpose (9), and folding
    // the thing they just selected under them would be maddening.
    if (!window.getSelection()?.isCollapsed) return;
    unfolded = !unfolded;
  }

  function foldKey(event: KeyboardEvent) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    unfolded = !unfolded;
  }
  $effect(() => {
    // Read once the words are on screen, and re-read when they change: a fold
    // is a fact about what was drawn, not a guess from the length of a string.
    void said;
    if (!body || !foldable) {
      folds = false;
      return;
    }
    folds = body.scrollHeight > FOLD_AT + 4;
  });
</script>

<!-- What a person said and what the agent answered are messages: they wrap,
     they carry their whole text, and the user's own turn is the green one. A
     tool call stays a single quiet line. tech.md 9 and 6.12. -->
{#if notice}
  <!-- Centred between two rules, the way Claude Code marks the same thing in
       its own transcript: it belongs to the conversation but nobody said it,
       so it takes neither side. tech.md 9. -->
  <div class="notice"><span>{entry.text}</span></div>
{:else if spoken}
  <div class="line" data-kind={entry.kind} data-state={entry.state}>
    <!-- It is a button exactly when it folds, and the checker cannot see a
         role decided at runtime: a message that fits is a message, and one
         that does not is the control that opens itself. tech.md 6.12. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="bubble"
      class:pressable={folds}
      role={folds ? 'button' : undefined}
      tabindex={folds ? 0 : undefined}
      aria-expanded={folds ? unfolded : undefined}
      aria-label={folds ? (unfolded ? 'Fold this message' : 'Open this message') : undefined}
      onclick={folds ? toggleFold : undefined}
      onkeydown={folds ? foldKey : undefined}
    >
      <div class="body" class:folded={folds && !unfolded} style:--fold={FOLD_AT} bind:this={body}>
        {#if shown.length > 0 || withFiles.files.length > 0}
          <div class="shots">
            {#each shown as path (path)}
              <ShotBlock
                name={shotName(path)}
                src={shotSrc?.(path)}
                onopen={() => onopenshot?.(path)}
                onbroken={() => (broken = [...broken, path])}
              />
            {/each}
            {#each withFiles.files as path (path)}
              <FileBlock {path} />
            {/each}
          </div>
        {/if}
        {#each parts as block, index (index)}
          {#if block.kind === 'code'}
            <pre class="code"><code>{block.value}</code></pre>
          {:else if block.kind === 'heading'}
            <!-- A heading in a bubble is a line that leads, not a banner: the
                 window is 400 pixels wide and everything in it is one
                 conversation. Weight and a little air, no size ladder.
                 tech.md 6.12. -->
            <p class="head" data-level={Math.min(block.level, 3)}>
              {@render inline(block.pieces)}
            </p>
          {:else if block.kind === 'list'}
            {#if block.ordered}
              <ol class="list" start={block.start}>
                {#each block.items as item, at (at)}
                  <li data-depth={item.depth}>{@render inline(item.pieces)}</li>
                {/each}
              </ol>
            {:else}
              <ul class="list">
                {#each block.items as item, at (at)}
                  <li data-depth={item.depth}>{@render inline(item.pieces)}</li>
                {/each}
              </ul>
            {/if}
          {:else if block.kind === 'table'}
            <!-- Its own scroller, because a table is the one thing in a
                 message that cannot be made narrower by wrapping: a column
                 squeezed to one letter per line is not a table any more.
                 tech.md 6.12. -->
            <!-- `sheet` and not `grid`: Tailwind owns that name and paints its own
                 rules over anything wearing it. tech.md 9. -->
            <div class="sheet">
              <table>
                {#if block.head}
                  <thead>
                    <tr>
                      {#each block.head as cell, at (at)}
                        <th style:text-align={block.align[at]}>{@render inline(cell)}</th>
                      {/each}
                    </tr>
                  </thead>
                {/if}
                <tbody>
                  {#each block.rows as row, at (at)}
                    <tr>
                      {#each row as cell, column (column)}
                        <td style:text-align={block.align[column]}>{@render inline(cell)}</td>
                      {/each}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {:else}
            <p class="prose">{@render inline(block.pieces)}</p>
          {/if}
        {/each}
      </div>
    </div>
  </div>
{:else}
  <div class="object" data-kind={entry.kind}>
    <button
      class="row"
      type="button"
      disabled={!entry.detail}
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <span class="dot" data-state={entry.state}></span>
      {#if entry.tool}
        <span class="tool">{entry.tool}</span>
      {/if}
      <span class="text">{entry.text}</span>
      {#if entry.detail}
        <span class="chevron" class:open aria-hidden="true">
          <svg viewBox="0 0 8 12" width="7" height="10">
            <path d="M1.5 1l5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.5" />
          </svg>
        </span>
      {/if}
    </button>
    {#if open && entry.detail}
      <pre class="detail">{entry.detail}</pre>
    {/if}
  </div>
{/if}

<!-- One run of inline spans, wherever it stands: a paragraph, a heading, a
     list item or a table cell. Never `{@html}`: this text comes out of an
     agent turn into a window over the whole screen. tech.md 6.12. -->
{#snippet inline(spans: Piece[])}
  {#each spans as piece, at (at)}
    {#if piece.kind === 'code'}
      <code class="inline">{piece.value}</code>
    {:else if piece.kind === 'bold'}
      <strong>{piece.value}</strong>
    {:else}
      {piece.value}
    {/if}
  {/each}
{/snippet}

<style>
  /* A message is there to be read and quoted, so it is one of the few places
     the island lets a selection happen at all. tech.md 9. */
  .bubble,
  .prose,
  .code,
  .detail,
  .text {
    -webkit-user-select: text;
    user-select: text;
  }

  .line {
    display: flex;
    padding: 3px 2px;
  }

  /* Folded, it fades out rather than stopping: a message cut on a hard edge
     reads as one that lost its end, and the fade says there is more. The mask
     works on any ground, which matters because this bubble is green and the
     one under it is not. tech.md 6.12. */
  .body.folded {
    max-height: calc(var(--fold) * 1px);
    overflow: hidden;
    -webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - 42px), transparent);
    mask-image: linear-gradient(to bottom, #000 calc(100% - 42px), transparent);
  }

  /* The message is the control, so it says so the only way a message can
     without growing a word: the pointer. tech.md 6.12. */
  .bubble.pressable {
    cursor: pointer;
  }

  .bubble {
    max-width: 84%;
    padding: 7px 10px;
    border-radius: 12px;
    font-size: 13px;
    line-height: 1.45;
    /* The whole message, wrapped. Cutting it with an ellipsis was the reason
       the dialogue could not be read at all. */
    overflow-wrap: anywhere;
    min-width: 0;
  }

  /* The user's own turn, in the product's green, on the side a messenger puts
     it. tech.md 9. */
  .line[data-kind='User'] {
    justify-content: flex-end;
  }

  .line[data-kind='User'] .bubble {
    background: var(--brand);
    color: var(--notch);
    border-bottom-right-radius: 4px;
  }

  /* Queued, not delivered. It leaves on the next stop of this session, and
     until then saying so quietly beats pretending it went. tech.md 6.5. */
  .line[data-kind='User'][data-state='Running'] .bubble {
    opacity: 0.55;
  }

  /* It will never leave, and a message that sits dim forever is worse than one
     that says so. */
  .line[data-kind='User'][data-state='Failed'] .bubble {
    background: var(--danger);
    opacity: 0.85;
  }

  /* The agent's own ground. On pure black a translucent surface has no edge,
     and the message reads as loose text. tech.md 9. */
  .line[data-kind='Assistant'] .bubble {
    background: var(--bubble);
    border: 1px solid var(--hairline);
    color: var(--text);
    border-bottom-left-radius: 4px;
  }

  /* A line about the conversation rather than in it. The rules are drawn by
     the row itself and grow to fill whatever the words leave. */
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 2px;
    color: var(--text-dim);
    font-size: 11px;
  }

  .notice::before,
  .notice::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--hairline);
  }

  .notice span {
    flex: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* An answer that is the API refusing, not the model speaking. The edge
     says so; the words inside already do. tech.md 6.11. */
  .line[data-kind='Assistant'][data-state='Failed'] .bubble {
    border-color: var(--danger);
  }

  .prose {
    margin: 0;
    white-space: pre-wrap;
  }

  /* One rule for every pair of blocks in a message rather than a list of
     pairs: a heading, a list and a table are three more kinds, and the list
     of pairs was already missing half of its own combinations. */
  .body > * + * {
    margin-top: 6px;
  }

  /* A heading is a line that leads. Weight and a little air above it, no
     size ladder: the bubble is 400 pixels wide and everything in it belongs
     to one conversation, so an `h1` drawn as an `h1` is a banner in a chat.
     tech.md 6.12. */
  .head {
    margin: 0;
    font-weight: 650;
    line-height: 1.35;
  }

  .head[data-level='1'],
  .head[data-level='2'] {
    font-size: 13px;
  }

  .head[data-level='3'] {
    font-size: 12px;
    opacity: 0.85;
  }

  .body > .head + * {
    margin-top: 3px;
  }

  .body > * + .head {
    margin-top: 10px;
  }

  /* The markers are asked for by name because the preflight of the CSS
     framework (3) takes them off every list on the page, and a list with no
     bullets is a run of indented lines. */
  .list {
    margin: 0;
    padding-left: 18px;
    line-height: 1.45;
  }

  ul.list {
    list-style: disc outside;
  }

  ol.list {
    list-style: decimal outside;
  }

  .list li + li {
    margin-top: 2px;
  }

  /* Indented rather than renested: a nested list arrives as items that were
     written deeper, and one padding step says exactly that without a second
     list inside the first. */
  .list li[data-depth='1'] {
    margin-left: 14px;
  }

  .list li[data-depth='2'] {
    margin-left: 28px;
  }

  .list li::marker {
    color: var(--text-dim);
  }

  .line[data-kind='User'] .list li::marker {
    color: rgba(0, 0, 0, 0.45);
  }

  /* A table is the one thing in a message that cannot be made narrower by
     wrapping, so it scrolls inside the bubble instead of widening it. */
  .sheet {
    overflow-x: auto;
    max-width: 100%;
  }

  .sheet table {
    border-collapse: collapse;
    font-size: 12px;
    line-height: 1.35;
  }

  .sheet th,
  .sheet td {
    padding: 4px 9px;
    border: 1px solid var(--hairline);
    vertical-align: top;
  }

  .sheet th {
    font-weight: 600;
    background: rgba(255, 255, 255, 0.05);
  }

  /* The green bubble has its own ground, and a white hairline on it reads as
     a gap rather than a rule. Same swap the code spans make. */
  .line[data-kind='User'] .sheet th,
  .line[data-kind='User'] .sheet td {
    border-color: rgba(0, 0, 0, 0.22);
  }

  .line[data-kind='User'] .sheet th {
    background: rgba(0, 0, 0, 0.1);
  }

  .inline,
  .code {
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 12px;
  }

  .inline {
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.35);
    color: var(--code);
  }

  .code {
    margin: 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.45);
    color: var(--code);
    overflow-x: auto;
    white-space: pre;
  }

  /* The user's own bubble is already green, so code inside it reads by weight
     rather than by colour. */
  .line[data-kind='User'] .inline,
  .line[data-kind='User'] .code {
    background: rgba(0, 0, 0, 0.18);
    color: var(--notch);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: var(--row);
    padding: 0 4px;
    border: none;
    background: transparent;
    font: inherit;
    text-align: left;
    min-width: 0;
    cursor: pointer;
  }

  .row:disabled {
    cursor: default;
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  .row:focus,
  .row:focus-visible {
    outline: none;
  }

  .row:hover:not(:disabled) .text {
    color: var(--text);
  }

  .chevron {
    flex: none;
    color: var(--text-dim);
    display: flex;
    transition: transform 120ms ease-out;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  /* The body of an object, as the terminal prints it. */
  .detail {
    margin: 0 4px 6px 18px;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.45);
    border: 1px solid var(--hairline);
    color: var(--text-dim);
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    line-height: 1.5;
    max-height: 220px;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  /* Reasoning is a marker, so it stays out of the way until asked for. */
  .object[data-kind='Thought'] .text {
    font-style: italic;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  /* A call still in flight, one that reported success, and one that never
     reported at all by the time the turn ended. tech.md 6.3. */
  .dot[data-state='Running'] {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }
  .dot[data-state='Ok'] {
    background: transparent;
    border: 1px solid var(--text-dim);
  }
  /* What the reply carried, at the top of its own bubble: it is what the
     message is about, and the words under it are the ask. Each one is a
     reference rather than the picture -- `ShotBlock` -- and pressing it opens
     the same full view the chip above the field opens. tech.md 6.13. */
  .shots {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 6px;
  }

  .dot[data-state='Failed'] {
    background: var(--danger);
  }

  /* The tool name is an identifier, so it reads as one. */
  .tool {
    flex: none;
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
    font-size: 11px;
    color: var(--text-dim);
  }

  .text {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
