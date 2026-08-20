/**
 * S12 acceptance. A running Peekle has to look different from one that is not
 * running, the mark has to open the session list, and the status it carries
 * has to be the one the user is waiting on.
 */

import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import userEvent from '@testing-library/user-event';
import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';

import { restStatus } from '$lib/logic/rest';
import { REST_DROP, REST_PILL, shapeBounds } from '$lib/logic/shape';
import RestMark from '$lib/ui/RestMark.svelte';
import Shape from '$lib/ui/Shape.svelte';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { SessionStatus } from '$lib/types/generated/SessionStatus';

const card = (status: SessionStatus, id = 's1'): SessionCard => ({
  session: { session_id: id, cwd: '/Users/x/peekle', project: 'peekle' },
  title: 'Refactor the panel code',
  status,
  entries: [],
  updated_at: 0,
});

describe('the status the resting mark carries', () => {
  it('is idle when nothing is running', () => {
    expect(restStatus([])).toBe('idle');
    expect(restStatus([card('Idle'), card('Ended', 's2')])).toBe('idle');
  });

  it('is working while a session is working', () => {
    expect(restStatus([card('Ended'), card('Working', 's2')])).toBe('working');
  });

  /// Waiting outranks working: it is the one state the user has to act on.
  it('is waiting whenever any session waits on the user', () => {
    expect(restStatus([card('Working'), card('WaitingOnUser', 's2')])).toBe('waiting');
  });

  it('is total over any list of cards', () => {
    const statuses = fc.constantFrom<SessionStatus>('Working', 'WaitingOnUser', 'Idle', 'Ended');
    fc.assert(
      fc.property(fc.array(statuses), (list) => {
        const value = restStatus(list.map((status, index) => card(status, `s${index}`)));
        expect(['idle', 'working', 'waiting']).toContain(value);
      }),
    );
  });
});

describe('the collapsed shape', () => {
  it('drops below the notch so there is something to see and to hit', () => {
    const bounds = shapeBounds('Collapsed', { width: 200, height: 32 });
    expect(bounds.height).toBe(32 + REST_DROP);
    expect(bounds.width).toBe(200);
  });

  it('floats a small pill on a display with no bezel to hang from', () => {
    const bounds = shapeBounds('Collapsed', { width: 200, height: 0 });
    expect(bounds.width).toBe(REST_PILL.width);
    expect(bounds.height).toBe(REST_PILL.height);
  });
});

describe('RestMark', () => {
  it('opens the session list on a click', async () => {
    const onopen = vi.fn();
    render(RestMark, { props: { status: 'idle', onopen } });

    await userEvent.click(screen.getByRole('button'));
    expect(onopen).toHaveBeenCalledOnce();
  });

  it('names the status it is showing, so the colour is not the only signal', () => {
    for (const status of ['idle', 'working', 'waiting'] as const) {
      const { container, unmount } = render(RestMark, { props: { status, onopen: () => {} } });
      expect(container.querySelector('.mark')?.getAttribute('data-status')).toBe(status);
      unmount();
    }
  });
});

describe('Shape at rest', () => {
  const notch = { width: 200, height: 32 };
  const mark = createRawSnippet(() => ({ render: () => '<span>peekle</span>' }));

  it('shows the mark while collapsed, so a running Peekle is visible', () => {
    const { container } = render(Shape, { props: { view: 'Collapsed', notch, rest: mark } });

    expect(screen.getByText('peekle')).toBeInTheDocument();
    expect(container.querySelector('.rest')).toBeInstanceOf(HTMLElement);
  });

  it('puts the mark away on every open view, where the content takes over', async () => {
    const { container, rerender } = render(Shape, {
      props: { view: 'Collapsed', notch, rest: mark },
    });

    for (const view of ['Pill', 'Sessions', { Session: 'abc' }] as IslandView[]) {
      await rerender({ view, notch, rest: mark });
      expect(container.querySelector('.rest')).toBeNull();
    }
  });

  it('does not invent a mark when it is given none', () => {
    const { container } = render(Shape, { props: { view: 'Collapsed', notch } });
    expect(container.querySelector('.rest')).toBeNull();
  });
});
