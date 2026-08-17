<script lang="ts">
  import type { TaskItem } from '$lib/types/generated/TaskItem';
  import LabelPill from './LabelPill.svelte';

  let { task }: { task: TaskItem } = $props();
</script>

<div class="row">
  <span class="dot" data-status={task.status}></span>
  <span class="title">{task.title}</span>
  <LabelPill label={task.label} />
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--row);
    padding: 0 4px;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--text-dim);
  }

  .dot[data-status='Active'] {
    background: var(--accent);
  }

  .dot[data-status='Done'] {
    background: transparent;
    border: 1px solid var(--text-dim);
  }

  .title {
    flex: 1;
    font-size: 13px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
