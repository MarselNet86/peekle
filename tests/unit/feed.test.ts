/**
 * Component tests for the feed row. S2 asks the row to show what a call is
 * and where it got to, so these check the state dot and the tool name rather
 * than the markup around them.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import FeedRow from '$lib/ui/FeedRow.svelte';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';

const entry = (over: Partial<FeedEntry> = {}): FeedEntry => ({
  id: '01J0',
  kind: 'Tool',
  text: 'cargo test --workspace',
  tool: 'Bash',
  detail: null,
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

  it("draws the user's own turn as a message rather than a row", () => {
    const { container } = render(FeedRow, {
      props: { entry: entry({ kind: 'User', tool: null, text: 'ship it', state: 'Ok' }) },
    });

    expect(container.querySelector('.tool')).toBeNull();
    expect(screen.getByText('ship it')).toBeInTheDocument();
    expect(container.querySelector('.line')?.getAttribute('data-kind')).toBe('User');
    expect(container.querySelector('.bubble')).toBeInstanceOf(HTMLElement);
  });

  it('renders a preview that is not ASCII without mangling it', () => {
    render(FeedRow, { props: { entry: entry({ text: 'проверить установку' }) } });
    expect(screen.getByText('проверить установку')).toBeInTheDocument();
  });
});

describe('what the agent said last', () => {
  it('carries the whole answer, because the feed scrolls now', () => {
    const long = Array.from({ length: 12 }, (_, i) => `line ${i + 1}`).join('\n');
    const { container } = render(FeedRow, {
      props: { entry: entry({ kind: 'Assistant', tool: null, text: long, state: 'Ok' }) },
    });

    expect(container.querySelector('.line')?.getAttribute('data-kind')).toBe('Assistant');
    // Every line is in the DOM. Clipping it to one row with an ellipsis was
    // the reason the dialogue could not be read. tech.md 6.12.
    expect(container.textContent).toContain('line 1');
    expect(container.textContent).toContain('line 12');
  });

  it('keeps a tool call on its single quiet line', () => {
    const { container } = render(FeedRow, { props: { entry: entry() } });
    expect(container.querySelector('.line')).toBeNull();
    expect(container.querySelector('.row')).toBeInstanceOf(HTMLElement);
  });
});

describe('an object with a body', () => {
  const call = entry({
    kind: 'Tool',
    tool: 'Bash',
    text: 'cargo test',
    detail: '{\n  "command": "cargo test"\n}\n\nok. 54 passed',
  });

  /// The terminal shows a collapsed line and opens it on demand. tech.md 6.12.
  it('keeps its body hidden until it is asked for', async () => {
    const { container } = render(FeedRow, { props: { entry: call } });

    expect(container.querySelector('.detail')).toBeNull();
    await userEvent.click(screen.getByRole('button'));
    expect(container.querySelector('.detail')?.textContent).toContain('ok. 54 passed');

    await userEvent.click(screen.getByRole('button'));
    expect(container.querySelector('.detail')).toBeNull();
  });

  it('offers nothing to open when there is no body', () => {
    render(FeedRow, { props: { entry: entry({ detail: null }) } });
    expect(screen.getByRole('button')).toBeDisabled();
  });

  it('draws a thought as its own marker', () => {
    const { container } = render(FeedRow, {
      props: {
        entry: entry({ kind: 'Thought', tool: null, text: 'Thought for 12s', detail: 'weighing' }),
      },
    });

    expect(container.querySelector('.object')?.getAttribute('data-kind')).toBe('Thought');
    expect(screen.getByText('Thought for 12s')).toBeInTheDocument();
  });
});
