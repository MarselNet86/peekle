<script lang="ts">
  let { text, collapsed = $bindable(true) }: { text: string; collapsed?: boolean } = $props();

  function toggle() {
    collapsed = !collapsed;
  }
</script>

<button class="block" class:collapsed onclick={toggle} type="button">
  {text}
</button>

<style>
  .block {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: transparent;
    padding: 0;
    color: var(--text-dim);
    font: inherit;
    font-size: 13px;
    line-height: 19px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    cursor: default;
  }

  .collapsed {
    display: -webkit-box;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
