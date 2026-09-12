<script lang="ts">
  import { accountCopy, canAct, fixCopy, runCopy } from '$lib/logic/signin';
  import Button from '$lib/ui/Button.svelte';
  import type { SignInState } from '$lib/types/generated/SignInState';
  import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

  // Named `signIn`, not `state`: a local called `state` collides with the
  // `$state` rune and Svelte reads `$state.url` as a store subscription.
  let {
    signIn,
    reason = null,
    busy = false,
    compact = false,
    onaction,
    oncode,
    onopen,
    oncancel,
  }: {
    signIn: SignInState;
    /** Why the account is out of reach, which decides what the button does. */
    reason?: UsageUnavailable | null;
    busy?: boolean;
    /** The same panel in one column, tighter and left aligned, for the strip
     * under the session list. Not a shorter panel: a strip that can only say
     * `Signed out` swallows the verdict of the press that produced it, and a
     * press that changes nothing on screen is a press that was lost.
     * tech.md 6.16. */
    compact?: boolean;
    onaction?: () => void;
    oncode?: (code: string) => void;
    /** Opens the authorize page. Rust does it, because an anchor here would
     * navigate the overlay webview itself. tech.md 6.16. */
    onopen?: () => void;
    oncancel?: () => void;
  } = $props();

  let code = $state('');

  const run = $derived(runCopy(signIn));
  const rest = $derived(accountCopy(reason));
  // Claude Code itself is what is missing, and the screen becomes one
  // instruction: it outranks every other line the panel could show, because
  // none of them are true until this one is done. tech.md 6.16.
  const fix = $derived(signIn.fix ? fixCopy(signIn.fix) : null);
  const title = $derived(fix?.title ?? run?.title ?? rest?.title ?? 'Signed out');
  const line = $derived(fix ? fix.line : run ? run.line : (signIn.error ?? rest?.line ?? ''));
  const action = $derived(rest?.action ?? 'Sign in');

  /** Long enough to be read, short enough not to become the label. */
  const COPIED_MS = 1600;
  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyCommand() {
    const command = signIn.fix?.command;
    if (!command) return;
    // The webview is not a secure context on every platform, so the clipboard
    // API is tried and the old selection trick catches the rest. A command
    // nobody can copy is a command they have to retype by hand.
    try {
      await navigator.clipboard.writeText(command);
    } catch {
      const field = document.createElement('textarea');
      field.value = command;
      field.setAttribute('readonly', '');
      field.style.position = 'fixed';
      field.style.opacity = '0';
      document.body.append(field);
      field.select();
      try {
        document.execCommand('copy');
      } finally {
        field.remove();
      }
    }
    copied = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = false), COPIED_MS);
  }
  // A refusal offers nothing to press: every button here would repeat what
  // has already been tried. tech.md 6.16.
  const acts = $derived(canAct(signIn));

  function submit() {
    // An empty code is not a submission. Sending one would put the CLI's own
    // prompt through a round trip that answers nothing.
    if (!code.trim()) return;
    oncode?.(code);
    code = '';
  }
</script>

<!-- One screen and one strip, same markup: the strip is the panel in a
     single column, and everything the press can produce has somewhere to
     land. tech.md 6.16. -->
<div class="account" class:compact>
  {#if !compact}
    <!-- Peekle's own sign, the one the resting mark wears, and grey rather
         than green because grey is exactly what it means here. Not an icon
         borrowed from a warning dialog: this is the island saying it is the
         thing that is cut off. tech.md 9. -->
    <svg class="sign" viewBox="0 0 14 12" width="36" height="31" aria-hidden="true">
      <path d="M4 10.6L6.9 1.4" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
      <path d="M9.1 10.6L12 1.4" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
    </svg>
  {/if}

  <h2>{title}</h2>
  {#if line}
    <p>{line}</p>
  {/if}

  {#if signIn.fix}
    <!-- The one line that fixes it, laid out to be copied rather than read:
         the shell it belongs in above, the command itself in the box, and the
         copy beside it. tech.md 6.16. -->
    <div class="fix">
      <span class="where">{fix?.label}</span>
      <div class="command">
        <code>{signIn.fix.command}</code>
        <button class="copy" type="button" onclick={copyCommand}>
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
    </div>
  {/if}

  {#if signIn.needs_code}
    <input
      type="text"
      bind:value={code}
      placeholder="Code from the page"
      aria-label="Code from the page"
      spellcheck="false"
      autocomplete="off"
      onkeydown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          submit();
        }
      }}
    />
  {/if}

  {#if acts}
    <div class="row">
      {#if run}
        <Button label="Cancel" wide onclick={() => oncancel?.()} />
        {#if signIn.needs_code}
          <Button
            label="Continue"
            variant="connect"
            wide
            disabled={!code.trim()}
            {busy}
            onclick={submit}
          />
        {/if}
      {:else}
        <!-- The one thing worth pressing in this block, so it takes the whole
             width of it rather than being measured by its own word. 6.16. -->
        <Button label={action} variant="connect" wide {busy} onclick={() => onaction?.()} />
      {/if}
    </div>
  {/if}

  <!-- The rarer way out, kept quiet so it never competes with the action. -->
  {#if signIn.url && run}
    <button class="quiet" type="button" onclick={() => onopen?.()}>Open the page</button>
  {/if}
</div>

<style>
  .account {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    /* Short lines, and the reason the text is capped rather than the block:
       the axis stays put while the wrapping changes. */
    max-width: 340px;
  }

  /* The strip under the session list: one column, left aligned, small. The
     list is still the thing on screen, so the account takes the height it
     needs to be understood and no more. tech.md 6.16. */
  .account.compact {
    align-items: stretch;
    text-align: left;
    max-width: none;
    width: 100%;
  }

  .compact h2 {
    font-size: 13px;
  }

  .compact p {
    margin-top: 3px;
    font-size: 11px;
    line-height: 1.4;
  }

  .compact input {
    margin-top: 8px;
    text-align: left;
    padding: 7px 10px;
  }

  .compact .row {
    margin-top: 8px;
  }

  .compact .row :global(button) {
    font-size: 12px;
    padding: 8px 14px;
  }

  .compact .quiet {
    margin-top: 8px;
    align-self: center;
  }

  .sign {
    color: var(--text-dim);
    opacity: 0.5;
    margin-bottom: 20px;
  }

  h2 {
    margin: 0;
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text);
  }

  p {
    margin: 7px 0 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-dim);
  }

  /* The instruction block: quiet label, the command in a box of its own, and
     the copy inside it. Nothing here is a control the panel competes with --
     the command is the action. tech.md 6.16 and 9. */
  .fix {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    width: 100%;
    margin-top: 18px;
    text-align: left;
  }

  .where {
    font-size: 11px;
    letter-spacing: 0.01em;
    color: var(--text-dim);
  }

  .command {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--hairline);
    border-radius: 9px;
    background: var(--bubble);
    padding: 8px 8px 8px 11px;
  }

  code {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    color: var(--text);
    /* Selectable, because copy is the fallback of the copy button and not the
       other way round. */
    user-select: text;
  }

  .copy {
    flex: none;
    border: none;
    border-radius: 6px;
    background: var(--hairline);
    color: var(--text-dim);
    font: inherit;
    font-size: 11px;
    padding: 4px 9px;
    cursor: pointer;
  }

  .copy:hover {
    color: var(--text);
  }

  .copy:focus,
  .copy:focus-visible {
    outline: none;
  }

  .compact .fix {
    margin-top: 10px;
  }

  input {
    width: 100%;
    margin-top: 18px;
    border: 1px solid var(--hairline);
    border-radius: 9px;
    background: var(--bubble);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: center;
    padding: 9px 12px;
  }

  input::placeholder {
    color: var(--text-dim);
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  input:focus,
  input:focus-visible {
    outline: none;
  }

  input:focus {
    border-color: var(--text-dim);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 22px;
    width: 100%;
  }

  /* The one thing on this screen worth pressing, so it is sized to be worth
     pressing. The product's small controls live in rows of other controls;
     this one stands alone. */
  .row :global(button) {
    font-size: 13px;
    padding: 10px 22px;
    border-radius: 9px;
  }

  .quiet {
    margin-top: 14px;
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    font-size: 12px;
    color: var(--text-dim);
    cursor: pointer;
  }

  .quiet:hover {
    color: var(--text);
  }

  .quiet:focus,
  .quiet:focus-visible {
    outline: none;
  }
</style>
