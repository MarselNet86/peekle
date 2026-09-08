<script lang="ts">
  import Button from '$lib/ui/Button.svelte';
  import type { SignInState } from '$lib/types/generated/SignInState';

  // Named `signIn`, not `state`: a local called `state` collides with the
  // `$state` rune and Svelte reads `$state.url` as a store subscription.
  let {
    signIn,
    busy = false,
    waiting = '',
    onstart,
    oncode,
    onopen,
    oncancel,
  }: {
    signIn: SignInState;
    busy?: boolean;
    /** What the panel says while the CLI works, from the feature store. */
    waiting?: string;
    onstart?: () => void;
    oncode?: (code: string) => void;
    /** Opens the authorize page. Rust does it, because an anchor here would
     * navigate the overlay webview itself. tech.md 6.16. */
    onopen?: () => void;
    oncancel?: () => void;
  } = $props();

  let code = $state('');

  const running = $derived(
    signIn.stage === 'Starting' || signIn.stage === 'Waiting' || signIn.stage === 'Finishing',
  );

  function submit() {
    // An empty code is not a submission. Sending one would put the CLI's own
    // prompt through a round trip that answers nothing.
    if (!code.trim()) return;
    oncode?.(code);
    code = '';
  }
</script>

<!-- The way back from `NotLoggedIn`. Re-reading the Keychain cannot fix a
     credential that is missing or expired, so this runs Claude Code's own
     `auth login` instead of offering a Reconnect that could never work.
     tech.md 6.16. -->
<div class="signin">
  {#if !running}
    <Button
      label={busy ? 'Signing in' : 'Sign in'}
      variant="connect"
      {busy}
      wide
      onclick={() => onstart?.()}
    />
  {:else}
    <p class="waiting">{waiting}</p>

    <!-- The browser not opening is an ordinary outcome: another default
         browser, a refused `open`. Without the address on screen there is
         nowhere left to go. tech.md 6.16. -->
    {#if signIn.url}
      <button class="url" type="button" onclick={() => onopen?.()}>Open the sign-in page</button>
    {/if}

    {#if signIn.needs_code}
      <div class="field">
        <input
          type="text"
          bind:value={code}
          placeholder="Paste the code"
          aria-label="Paste the code"
          spellcheck="false"
          autocomplete="off"
          onkeydown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              submit();
            }
          }}
        />
      </div>
      <Button
        label="Continue"
        variant="connect"
        disabled={!code.trim()}
        {busy}
        wide
        onclick={submit}
      />
    {/if}

    <Button label="Cancel" onclick={() => oncancel?.()} wide />
  {/if}

  <!-- Why it did not work, in words. A press that changes nothing on screen
       reads as a press that was lost. tech.md 6.4. -->
  {#if signIn.error}
    <p class="why">{signIn.error}</p>
  {/if}
</div>

<style>
  .signin {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: stretch;
  }

  .waiting {
    margin: 0;
    text-align: center;
    font-size: 12px;
    color: var(--text);
  }

  .url {
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    text-align: center;
    font-size: 11px;
    color: var(--accent);
    cursor: pointer;
  }

  .url:hover {
    text-decoration: underline;
  }

  .url:focus,
  .url:focus-visible {
    outline: none;
  }

  .field {
    display: flex;
    align-items: center;
    padding: 5px 8px;
    border-radius: 9px;
    background: var(--bubble);
  }

  input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    padding: 0;
  }

  input::placeholder {
    color: var(--text-dim);
  }

  /* No focus rings anywhere in the island. tech.md 9. */
  input:focus,
  input:focus-visible {
    outline: none;
  }

  .why {
    margin: 0;
    text-align: center;
    font-size: 11px;
    color: var(--text-dim);
  }
</style>
