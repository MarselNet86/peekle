<script lang="ts">
  import type { TaskLabel } from '$lib/types/generated/TaskLabel';

  let { label }: { label: TaskLabel } = $props();
</script>

<span class="pill" data-label={label}>{label}</span>

<style>
  .pill {
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 4px;
    border: 1px solid var(--hairline);
    color: var(--text-dim);
    white-space: nowrap;
  }

  .pill[data-label='Bug'] {
    color: var(--danger);
  }
  .pill[data-label='Fix'] {
    color: var(--orange);
  }
  .pill[data-label='Test'] {
    color: var(--accent);
  }
  .pill[data-label='Docs'] {
    color: var(--warn);
  }
  .pill[data-label='Research'] {
    color: #7db6d8;
  }
  .pill[data-label='Chore'] {
    color: var(--text-dim);
  }
  .pill[data-label='Code'] {
    color: var(--text);
  }
</style>
