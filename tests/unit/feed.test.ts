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

describe('FeedRow draws the formatting an answer is written in', () => {
  const answer = (text: string) =>
    render(FeedRow, {
      props: { entry: entry({ kind: 'Assistant', tool: null, state: 'Ok', text }) },
    });

  /// The pipes were drawn as pipes until v80.18: an answer with a table in it
  /// arrived as a wall of dashes and bars. tech.md 6.12.
  it('draws a table as a table, with a cell per column', () => {
    const { container } = answer('| Cost |\n| ---: |\n| 1908 |');

    const table = container.querySelector('table');
    expect(table).not.toBeNull();
    expect(screen.getByText('Cost').tagName).toBe('TH');
    const cell = screen.getByText('1908');
    expect(cell.tagName).toBe('TD');
    expect(cell.style.textAlign).toBe('right');
  });

  /// Written by hand, an empty heading row means "just the grid".
  it('draws no heading band when the heading row was empty', () => {
    const { container } = answer('| | |\n|---|---|\n| Paid | 1908 |');

    expect(container.querySelector('thead')).toBeNull();
    expect(container.querySelectorAll('tbody td')).toHaveLength(2);
  });

  it('draws a run of bullets as a list, and a numbered one from its own number', () => {
    const { container } = answer('- one\n- two');
    expect(container.querySelectorAll('ul.list li')).toHaveLength(2);

    const numbered = answer('3. third\n4. fourth');
    const ordered = numbered.container.querySelector('ol.list');
    expect(ordered?.getAttribute('start')).toBe('3');
  });

  it('draws a heading as a line that leads rather than as hashes', () => {
    const { container } = answer('## What changed\nthe row moved');

    const head = container.querySelector('.head');
    expect(head?.textContent).toBe('What changed');
    expect(screen.queryByText(/##/)).toBeNull();
  });

  /// A shell command is full of pipes and a message about flags is full of
  /// dashes. Neither is a table or a list. tech.md 6.12.
  it('leaves a sentence that merely carries a pipe or a dash alone', () => {
    const { container } = answer('ls | grep peekle\n-not a bullet');

    expect(container.querySelector('table')).toBeNull();
    expect(container.querySelector('.list')).toBeNull();
    expect(screen.getByText(/ls \| grep peekle/)).toBeInTheDocument();
  });
});

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

  /// A model change belongs to the conversation but nobody said it, so it
  /// takes neither side: no bubble, no tool row, a line of its own.
  /// tech.md 6.15 and 9.
  it('draws a model switch as a line about the conversation, not in it', () => {
    const { container } = render(FeedRow, {
      props: {
        entry: entry({
          kind: 'Notice',
          tool: null,
          state: 'Ok',
          text: 'Switched to claude-opus-5[1m]',
        }),
      },
    });

    expect(screen.getByText('Switched to claude-opus-5[1m]')).toBeInTheDocument();
    expect(container.querySelector('.notice')).not.toBeNull();
    expect(container.querySelector('.bubble')).toBeNull();
    expect(container.querySelector('.row')).toBeNull();
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

describe('a reply on its way out', () => {
  it('reads as queued until something carries it', () => {
    const { container } = render(FeedRow, {
      props: { entry: entry({ kind: 'User', tool: null, text: 'keep going', state: 'Running' }) },
    });
    expect(container.querySelector('.line')?.getAttribute('data-state')).toBe('Running');
  });

  /// A message nobody could deliver says so rather than sitting dim forever.
  it('reads as undelivered when no turn could be started', () => {
    const { container } = render(FeedRow, {
      props: { entry: entry({ kind: 'User', tool: null, text: 'keep going', state: 'Failed' }) },
    });
    expect(container.querySelector('.line')?.getAttribute('data-state')).toBe('Failed');
  });
});
