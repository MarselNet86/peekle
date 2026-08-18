/**
 * Component tests for the feed row. S2 asks the row to show what a call is
 * and where it got to, so these check the state dot and the tool name rather
 * than the markup around them.
 */

import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import FeedRow from '$lib/ui/FeedRow.svelte';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';

const entry = (over: Partial<FeedEntry> = {}): FeedEntry => ({
  id: '01J0',
  kind: 'Tool',
  text: 'cargo test --workspace',
  tool: 'Bash',
  state: 'Running',
  at: 0,
  ...over,
});

function dotOf(container: HTMLElement): HTMLElement {
  const dot = container.querySelector('.dot');
  if (!(dot instanceof HTMLElement)) throw new Error('the row drew no state dot');
  return dot;
}

describe('FeedRow', () => {
  it('names the tool and previews the call', () => {
    render(FeedRow, { props: { entry: entry() } });

    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('cargo test --workspace')).toBeInTheDocument();
  });

  it('carries every state on the dot', () => {
    for (const state of ['Running', 'Ok', 'Failed'] as const) {
      const { container, unmount } = render(FeedRow, { props: { entry: entry({ state }) } });
      expect(dotOf(container).dataset.state).toBe(state);
      unmount();
    }
  });

  it('leaves the tool name out of a turn that is not a tool call', () => {
    const { container } = render(FeedRow, {
      props: { entry: entry({ kind: 'User', tool: null, text: 'ship it', state: 'Ok' }) },
    });

    expect(container.querySelector('.tool')).toBeNull();
    expect(screen.getByText('ship it')).toBeInTheDocument();
    expect(container.querySelector('.row')?.getAttribute('data-kind')).toBe('User');
  });

  it('renders a preview that is not ASCII without mangling it', () => {
    render(FeedRow, { props: { entry: entry({ text: 'проверить установку' }) } });
    expect(screen.getByText('проверить установку')).toBeInTheDocument();
  });
});
