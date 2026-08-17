<script lang="ts">
  import type { Snippet } from 'svelte';

  type Tone = 'prompt' | 'hud' | 'island';

  let {
    tone = 'prompt',
    padded = true,
    children,
  }: { tone?: Tone; padded?: boolean; children: Snippet } = $props();
</script>

<div class="panel" class:padded data-tone={tone}>
  {@render children()}
</div>

<style>
  .panel {
    background: var(--surface);
    backdrop-filter: blur(var(--blur)) saturate(140%);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    color: var(--text);
    overflow: hidden;
  }

  .panel[data-tone='hud'],
  .panel[data-tone='island'] {
    background: var(--surface-hud);
  }

  .panel[data-tone='island'] {
    border-radius: 999px;
  }

  .padded {
    padding: 12px 14px;
  }
</style>
