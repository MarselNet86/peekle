<script lang="ts">
  import { accountCopy, runCopy } from '$lib/logic/signin';
  import Button from '$lib/ui/Button.svelte';
  import type { SignInState } from '$lib/types/generated/SignInState';
  import type { UsageUnavailable } from '$lib/types/generated/UsageUnavailable';

  // Named `signIn`, not `state`: a local called `state` collides with the
  // `$state` rune and Svelte reads `$state.url` as a store subscription.
  let {
    signIn,
    reason = null,
    busy = false,
    onaction,
    oncode,
    onopen,
    oncancel,
  }: {
    signIn: SignInState;
    /** Why the account is out of reach, which decides what the button does. */
    reason?: UsageUnavailable | null;
    busy?: boolean;
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
  const title = $derived(run?.title ?? rest?.title ?? 'Signed out');
  const line = $derived(run ? run.line : (signIn.error ?? rest?.line ?? ''));

  function submit() {
    // An empty code is not a submission. Sending one would put the CLI's own
    // prompt through a round trip that answers nothing.
    if (!code.trim()) return;
    oncode?.(code);
    code = '';
  }
</script>

<!-- The way back into the account. Two lines and one action: a screen that
     stands in front of someone's work earns its space by being short.
     tech.md 6.16. -->
<div class="account">
  <h2>{title}</h2>
  {#if line}
    <p>{line}</p>
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

  <div class="row">
    <!-- The browser not opening is ordinary: another default browser, a
         refused `open`. Kept quiet, because it is the rarer way out. -->
    {#if signIn.url && run}
      <button class="quiet" type="button" onclick={() => onopen?.()}>Open the page</button>
    {/if}
    <span class="gap"></span>

    {#if run}
      <Button label="Cancel" onclick={() => oncancel?.()} />
      {#if signIn.needs_code}
        <Button
          label="Continue"
          variant="connect"
          disabled={!code.trim()}
          {busy}
          onclick={submit}
        />
      {/if}
    {:else}
      <Button
        label={rest?.action ?? 'Sign in'}
        variant="connect"
        {busy}
        onclick={() => onaction?.()}
      />
    {/if}
  </div>
</div>

<style>
  .account {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    max-width: 300px;
  }

  /* The one line that carries weight here, and the only place in the island
     that goes above 13px: everything else on this screen is support. */
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--text);
  }

  p {
    margin: 0;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-dim);
  }

  input {
    margin-top: 4px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--bubble);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    padding: 8px 10px;
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
    gap: 8px;
    margin-top: 8px;
  }

  .gap {
    flex: 1;
  }

  /* A way out that is not the way out: plain text, no border, so it never
     competes with the action beside it. */
  .quiet {
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    font-size: 11px;
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
