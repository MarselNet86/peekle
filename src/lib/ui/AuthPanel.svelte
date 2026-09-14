<script lang="ts">
  import { authCopy, authScreen } from '$lib/logic/account';
  import { SIGN_BOX, SIGN_STROKES, SIGN_WEIGHT } from '$lib/logic/sign';
  import Button from './Button.svelte';
  import CommandLine from './CommandLine.svelte';
  import type { AccountLink } from '$lib/types/generated/AccountLink';
  import type { AccountState } from '$lib/types/generated/AccountState';
  import type { SignInState } from '$lib/types/generated/SignInState';

  // Named `signIn`, not `state`: a local called `state` collides with the
  // `$state` rune.
  let {
    account,
    signIn,
    busy = false,
    checking = false,
    copied = false,
    onsignin,
    oncode,
    onopen,
    oncancel,
    oncheck,
    oncopy,
    onlink,
  }: {
    /** What Claude Code on this Mac can do. Null before the CLI has been
     * asked, and then the window reads as a plain sign-in. tech.md 6.16. */
    account: AccountState | null;
    signIn: SignInState;
    busy?: boolean;
    /** `Check again` is asking the CLI. */
    checking?: boolean;
    /** The command was just copied. */
    copied?: boolean;
    onsignin?: () => void;
    oncode?: (code: string) => void;
    /** Opens the page with the code again. Rust does it: an anchor would
     * navigate the overlay itself. tech.md 6.16. */
    onopen?: () => void;
    oncancel?: () => void;
    oncheck?: () => void;
    oncopy?: () => void;
    onlink?: (link: AccountLink) => void;
  } = $props();

  let codeOpen = $state(false);
  let code = $state('');

  const screen = $derived(authScreen(account, signIn));
  const copy = $derived(authCopy(screen, account, signIn));
  const signHeight = Math.round((28 * SIGN_BOX.height) / SIGN_BOX.width);

  // A code field belongs to the run it was opened in. tech.md 6.16.
  $effect(() => {
    if (screen !== 'waiting') {
      codeOpen = false;
      code = '';
    }
  });

  function submit() {
    // An empty code is not a submission.
    const value = code.trim();
    if (!value) return;
    oncode?.(value);
    code = '';
  }
</script>

<!-- One axis down the middle, the way a macOS sheet stands: the tile, the
     title, the line, one action and the quiet ways out under it. A screen that
     changes swaps its content and keeps its shape. tech.md 6.16. -->
<div class="auth" data-screen={screen}>
  {#key screen}
    <div class="stage">
      <div
        class="tile"
        class:breathing={screen === 'waiting' || screen === 'starting'}
        class:done={screen === 'done'}
        class:denied={screen === 'denied'}
        class:quiet={screen === 'install' || screen === 'update' || screen === 'failed'}
        aria-hidden="true"
      >
        {#if screen === 'done'}
          <svg viewBox="0 0 24 24" width="26" height="26">
            <path
              class="check"
              d="M5 12.5l4.2 4.2L19 7"
              fill="none"
              stroke="currentColor"
              stroke-width="2.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        {:else}
          <svg viewBox="0 0 {SIGN_BOX.width} {SIGN_BOX.height}" width="28" height={signHeight}>
            {#each SIGN_STROKES as stroke (stroke)}
              <path
                d={stroke}
                stroke="currentColor"
                stroke-width={SIGN_WEIGHT}
                stroke-linecap="round"
                fill="none"
              />
            {/each}
          </svg>
        {/if}
      </div>

      <h2>{copy.title}</h2>
      {#if copy.line}
        <p>{copy.line}</p>
      {/if}

      {#if screen === 'install' || screen === 'update'}
        {#if account?.command}
          <div class="action full">
            <CommandLine command={account.command} {copied} oncopy={() => oncopy?.()} />
          </div>
        {/if}
        <div class="action second">
          <Button label="Check again" busy={checking} onclick={() => oncheck?.()} />
        </div>
        <div class="links">
          {#if screen === 'install'}
            <button type="button" class="link" onclick={() => onlink?.('InstallGuide')}>
              Installation guide
            </button>
          {:else}
            <button type="button" class="link" onclick={() => onlink?.('Changelog')}>
              What's new
            </button>
            <button type="button" class="link" onclick={() => onlink?.('UpdateGuide')}>
              Update guide
            </button>
          {/if}
        </div>
      {:else if screen === 'signin'}
        <div class="action">
          <Button
            label="Sign in with Claude"
            variant="prominent"
            wide
            {busy}
            onclick={() => onsignin?.()}
          />
        </div>
      {:else if screen === 'starting'}
        <div class="dots" role="status" aria-label="Opening your browser">
          <span></span><span></span><span></span>
        </div>
        <div class="links">
          <button type="button" class="link" onclick={() => oncancel?.()}>Cancel</button>
        </div>
      {:else if screen === 'waiting'}
        {#if codeOpen}
          <!-- The rarer path: the page showed a code instead of coming back
               on its own. Hidden until asked for, because a field standing
               here tells a person to paste a code they do not have. 6.16. -->
          <form
            class="code"
            onsubmit={(event) => {
              event.preventDefault();
              submit();
            }}
          >
            <input
              type="text"
              bind:value={code}
              placeholder="Paste the code"
              aria-label="Paste the code"
              spellcheck="false"
              autocomplete="off"
            />
            <Button
              label="Continue"
              variant="connect"
              disabled={!code.trim()}
              {busy}
              onclick={submit}
            />
          </form>
        {:else}
          <div class="dots" role="status" aria-label="Waiting for your browser">
            <span></span><span></span><span></span>
          </div>
        {/if}
        <div class="links">
          {#if signIn.url}
            <button type="button" class="link" onclick={() => onopen?.()}>
              Open the page again
            </button>
          {/if}
          {#if !codeOpen}
            <button type="button" class="link" onclick={() => (codeOpen = true)}>
              Have a code?
            </button>
          {/if}
          <button type="button" class="link" onclick={() => oncancel?.()}>Cancel</button>
        </div>
      {:else if screen === 'finishing'}
        <span class="spinner" role="status" aria-label="Signing in"></span>
      {:else if screen === 'denied' || screen === 'failed'}
        <div class="action">
          <Button label="Try again" variant="prominent" wide {busy} onclick={() => onsignin?.()} />
        </div>
      {/if}
    </div>
  {/key}
</div>

<style>
  .auth {
    display: flex;
    justify-content: center;
    width: 100%;
  }

  /* Every screen arrives the same way: up from eight pixels below, on the
     curve every entrance in the product rides. Opacity and offset only.
     tech.md 6.10 and 6.16. */
  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    width: 100%;
    /* Wide enough for a command to stand whole on one line: a command cut at
       the edge is a command nobody can read before pasting it. The words stay
       narrow below. tech.md 6.16. */
    max-width: 420px;
    animation: arrive 280ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .tile {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    margin-bottom: 16px;
    border: 1px solid var(--hairline);
    border-radius: 15px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.09), rgba(255, 255, 255, 0.035));
    color: var(--brand);
  }

  .tile.quiet {
    color: var(--text-dim);
  }

  /* Waiting on a browser: the tile breathes, slowly, and nothing else moves
     that is not the dots. */
  .tile.breathing {
    animation: breathe 2400ms ease-in-out infinite;
  }

  @keyframes breathe {
    0%,
    100% {
      transform: scale(1);
    }
    50% {
      transform: scale(1.04);
    }
  }

  .tile.done {
    border-color: rgba(48, 209, 88, 0.35);
    background: var(--brand-soft);
    animation: pop 520ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes pop {
    0% {
      transform: scale(0.86);
    }
    60% {
      transform: scale(1.06);
    }
    100% {
      transform: scale(1);
    }
  }

  .check {
    stroke-dasharray: 20;
    stroke-dashoffset: 20;
    animation: draw 420ms cubic-bezier(0.22, 1, 0.36, 1) 120ms forwards;
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  /* A refusal shakes once, the way a declined password field does. */
  .tile.denied {
    color: var(--text-dim);
    animation: shake 420ms cubic-bezier(0.36, 0.07, 0.19, 0.97);
  }

  @keyframes shake {
    20% {
      transform: translateX(-6px);
    }
    40% {
      transform: translateX(6px);
    }
    60% {
      transform: translateX(-4px);
    }
    80% {
      transform: translateX(4px);
    }
    100% {
      transform: none;
    }
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--text);
    text-wrap: balance;
  }

  p {
    max-width: 320px;
    text-wrap: balance;
    margin: 6px 0 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-dim);
  }

  .action {
    display: flex;
    justify-content: center;
    width: 100%;
    /* A button the width of the whole island reads as a banner, not a
       control: actions keep the width of the words above them. */
    max-width: 320px;
    margin-top: 20px;
  }

  /* The command is the exception, and the reason the block is wide at all. */
  .action.full {
    max-width: none;
  }

  .action.second {
    margin-top: 10px;
  }

  /* The one thing on the screen worth pressing, sized to be worth pressing.
     Direct children only: the copy button inside a command line stays its
     own size. */
  .action > :global(button) {
    font-size: 13px;
    padding: 10px 18px;
    border-radius: 10px;
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 16px;
    margin-top: 14px;
  }

  .link {
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    font-size: 12px;
    color: var(--text-dim);
    cursor: pointer;
    transition: color 120ms ease;
  }

  .link:hover {
    color: var(--text);
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  .link:focus,
  .link:focus-visible,
  input:focus,
  input:focus-visible {
    outline: none;
  }

  .dots {
    display: flex;
    gap: 6px;
    margin-top: 22px;
  }

  .dots span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-dim);
    animation: dot 1200ms ease-in-out infinite;
  }

  .dots span:nth-child(2) {
    animation-delay: 160ms;
  }

  .dots span:nth-child(3) {
    animation-delay: 320ms;
  }

  @keyframes dot {
    0%,
    80%,
    100% {
      opacity: 0.25;
      transform: scale(0.85);
    }
    40% {
      opacity: 1;
      transform: scale(1);
    }
  }

  .spinner {
    width: 18px;
    height: 18px;
    margin-top: 22px;
    box-sizing: border-box;
    border: 2px solid var(--hairline);
    border-top-color: var(--text);
    border-radius: 50%;
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .code {
    display: flex;
    gap: 8px;
    width: 100%;
    margin-top: 20px;
    animation: arrive 240ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  input {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--hairline);
    border-radius: 9px;
    background: var(--bubble);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    padding: 8px 11px;
    user-select: text;
    -webkit-user-select: text;
  }

  input::placeholder {
    color: var(--text-dim);
  }

  input:focus {
    border-color: var(--text-dim);
  }

  @media (prefers-reduced-motion: reduce) {
    .stage,
    .code,
    .tile,
    .dots span,
    .spinner {
      animation: none;
    }

    .check {
      animation: none;
      stroke-dashoffset: 0;
    }
  }
</style>
