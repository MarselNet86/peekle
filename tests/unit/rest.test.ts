/**
 * S12 acceptance. A running Peekle has to look different from one that is not
 * running, the mark has to open the session list, and the status it carries
 * has to be the one the user is waiting on.
 */

import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import userEvent from '@testing-library/user-event';
import fc from 'fast-check';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { clickPutsAway, clickSettles, restStatus } from '$lib/logic/rest';
import { REST_PILL, REST_SIDE, shapeBounds } from '$lib/logic/shape';
import RestMark from '$lib/ui/RestMark.svelte';
import Shape from '$lib/ui/Shape.svelte';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { SessionStatus } from '$lib/types/generated/SessionStatus';

const card = (status: SessionStatus, id = 's1'): SessionCard => ({
  session: { session_id: id, cwd: '/Users/x/peekle', project: 'peekle', pid: null, tty: null },
  title: 'Refactor the panel code',
  status,
  origin: 'Observed',
  entries: [],
  agent: null,
  mode: null,
  thinking: null,
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
  /// Only a permission request reaches it now. A finished turn holds its own
  /// channel open and needs nobody. tech.md 6.5.
  it('is waiting only while a permission request is open', () => {
    expect(restStatus([card('Working')], true)).toBe('waiting');
    expect(restStatus([card('Idle')], false)).toBe('idle');
  });

  it('is total over any list of cards', () => {
    const statuses = fc.constantFrom<SessionStatus>('Working', 'Idle', 'Ended');
    fc.assert(
      fc.property(fc.array(statuses), fc.boolean(), (list, prompt) => {
        const value = restStatus(
          list.map((status, index) => card(status, `s${index}`)),
          prompt,
        );
        expect(['idle', 'working', 'waiting']).toContain(value);
      }),
    );
  });
});

describe('the collapsed shape', () => {
  /// A notch is the absence of pixels, so a mark drawn across its width cannot
  /// be seen at all. The overhang is the whole point. tech.md 6.7.
  it('overhangs the notch on both sides and never below it', () => {
    const bounds = shapeBounds('Collapsed', { width: 200, height: 32 });
    expect(bounds.width).toBe(200 + 2 * REST_SIDE);
    expect(bounds.height).toBe(32);
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
    render(RestMark, { props: { status: 'idle', pct: 12, onopen } });

    await userEvent.click(screen.getByRole('button'));
    expect(onopen).toHaveBeenCalledOnce();
  });

  it('names the status it is showing, so the colour is not the only signal', () => {
    for (const status of ['idle', 'working', 'waiting'] as const) {
      const { container, unmount } = render(RestMark, {
        props: { status, pct: 12, onopen: () => {} },
      });
      expect(container.querySelector('.mark')?.getAttribute('data-status')).toBe(status);
      unmount();
    }
  });
});

describe('closing an open island with a click', () => {
  const built: HTMLElement[] = [];

  afterEach(() => {
    for (const node of built.splice(0)) node.remove();
  });

  // In the document, because a real click lands on a node that is in it, and
  // being out of it is the separate fact this rule now reads. tech.md 6.7.
  function targets() {
    const shape = document.createElement('div');
    shape.className = 'shape';
    const inside = document.createElement('button');
    shape.append(inside);
    const outside = document.createElement('div');
    document.body.append(shape, outside);
    built.push(shape, outside);
    return { shape, inside, outside };
  }

  it('closes on a click beside the shape', () => {
    const { outside } = targets();
    expect(clickPutsAway('Sessions', outside)).toBe(true);
    expect(clickPutsAway({ Session: 'abc' }, null)).toBe(true);
  });

  it('leaves a click on the shape alone, however deep it landed', () => {
    const { shape, inside } = targets();
    expect(clickPutsAway('Sessions', shape)).toBe(false);
    expect(clickPutsAway('Sessions', inside)).toBe(false);
  });

  /// Collapsing is not resolving. The hook stays pending either way, and rule
  /// 10 is about who resolves it, not about who may hide a window.
  it('closes even while a request is still waiting', () => {
    const { outside } = targets();
    expect(clickPutsAway('Sessions', outside)).toBe(true);
    expect(clickPutsAway({ Session: 'abc' }, outside)).toBe(true);
  });

  it('does nothing while the island is already resting', () => {
    const { outside } = targets();
    expect(clickPutsAway('Collapsed', outside)).toBe(false);
  });

  /// The cross on an attachment deletes its own chip, and Svelte applies that
  /// before the click reaches the window. A detached node has no ancestors at
  /// all, so it read as a click beside the shape and took the whole island
  /// with it. tech.md 6.7.
  it('leaves the island alone when the click removed its own target', () => {
    const { shape } = targets();
    const cross = document.createElement('button');
    shape.appendChild(cross);
    expect(clickPutsAway('Sessions', cross)).toBe(false);

    cross.remove();
    expect(clickPutsAway('Sessions', cross)).toBe(false);
    expect(clickSettles('Sessions', cross, false)).toBe('nothing');
  });

  /// A picture open at full size is what the click is aimed at, and collapsing
  /// would carry off the feed and the reply with it. tech.md 6.13.
  describe('with a shot open', () => {
    it('spends the click on the picture and keeps the island', () => {
      const { outside } = targets();
      expect(clickSettles('Sessions', outside, true)).toBe('preview');
    });

    it('puts the island away on the next one', () => {
      const { outside } = targets();
      expect(clickSettles('Sessions', outside, false)).toBe('island');
    });

    it('leaves a click on the shape alone either way', () => {
      const { inside } = targets();
      expect(clickSettles('Sessions', inside, true)).toBe('nothing');
      expect(clickSettles('Sessions', inside, false)).toBe('nothing');
    });
  });
});

describe('the limit ring', () => {
  const ring = (container: HTMLElement) => container.querySelector('.dial[data-tone]');

  it('takes its colour from the thresholds UsageBar uses', () => {
    for (const [pct, tone] of [
      [12, 'accent'],
      [62, 'warn'],
      [81, 'orange'],
      [96, 'danger'],
    ] as [number, string][]) {
      const { container, unmount } = render(RestMark, {
        props: { status: 'idle', pct, onopen: () => {} },
      });
      expect(ring(container)?.getAttribute('data-tone')).toBe(tone);
      unmount();
    }
  });

  /// The numbers come off undocumented headers, so an unknown one draws an
  /// empty ring. A zero would be a claim nobody made. tech.md R-3.
  it('draws nothing at all rather than a zero it was never told', () => {
    const { container } = render(RestMark, {
      props: { status: 'idle', pct: null, onopen: () => {} },
    });

    expect(ring(container)).toBeNull();
    expect(container.querySelector('.dial')).toBeInstanceOf(HTMLElement);
    expect(container.querySelector('.dial .fill')).toBeNull();
  });

  it('says the number out loud, so the colour is not the only signal', () => {
    render(RestMark, { props: { status: 'waiting', pct: 62, onopen: () => {} } });
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(
      /waiting on you, 62% of the 5h window used/,
    );
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
