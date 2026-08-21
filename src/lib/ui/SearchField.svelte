<script lang="ts">
  let {
    value = $bindable(''),
    placeholder = 'Search sessions…',
  }: { value?: string; placeholder?: string } = $props();
</script>

<!-- No autofocus. The island opens without the keyboard and keeps it that way
     until the user asks: a search box that grabs focus would take the keys out
     of whatever they were doing. tech.md 6.7. -->
<div class="search">
  <svg viewBox="0 0 14 14" width="12" height="12" aria-hidden="true">
    <circle cx="6" cy="6" r="4.2" fill="none" stroke="currentColor" stroke-width="1.4" />
    <path d="M9.2 9.2L12.6 12.6" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
  </svg>
  <input type="text" bind:value {placeholder} aria-label="Search sessions" spellcheck="false" />
</div>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: none;
    padding: 5px 8px;
    border-radius: 9px;
    background: var(--bubble);
    color: var(--text-dim);
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
</style>
