/**
 * What a compact looks like while it runs and when it is over. tech.md 6.21.
 *
 * A compact takes minutes, the agent answers nothing through it, and before
 * this the island said nothing at all about it: the notch stood green and
 * empty and the dialogue stood still. So the acceptance is about being able
 * to tell, from across a screen, that something is happening and that it has
 * stopped happening.
 */

import { render, screen } from '@testing-library/svelte';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { restStatus } from '$lib/logic/rest';
import { ASKED_TO_STOP } from '$lib/logic/sessions';
import { feedRows } from '$lib/logic/work';
import RestMark from '$lib/ui/RestMark.svelte';
import WorkLine from '$lib/ui/WorkLine.svelte';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { SessionStatus } from '$lib/types/generated/SessionStatus';

const card = (
  status: SessionStatus,
  compacting: SessionCard['compacting'] = null,
  id = 's1',
): SessionCard => ({
  session: { session_id: id, cwd: '/Users/x/peekle', project: 'peekle', pid: null, tty: null },
  title: 'Refactor the panel code',
  status,
  origin: 'Observed',
  entries: [],
  agent: null,
  mode: null,
  thinking: null,
  compacting,
  stopping: null,
  updated_at: 0,
});

const running = { since: 1_000, manual: true };

describe('the mark while a compact runs', () => {
  it('says compacting, however the session itself stands', () => {
    expect(restStatus([card('Idle', running)])).toBe('compacting');
    expect(restStatus([card('Working', running)])).toBe('compacting');
  });

  /// A compact is not ordinary work: it runs for minutes and the agent
  /// answers nothing through it, so the spinner of a turn over it would say
  /// the usual thing is happening. tech.md 6.21.
  it('outranks a session that is merely working', () => {
    expect(restStatus([card('Working'), card('Idle', running, 's2')])).toBe('compacting');
  });

  /// Waiting is the one state that costs the user time, and it keeps the
  /// mark whatever else is going on. tech.md 6.7.
  it('gives way to a request that is waiting on the user', () => {
    expect(restStatus([card('Idle', running)], true)).toBe('waiting');
  });

  it('goes back to what the sessions say the moment the compact ends', () => {
    expect(restStatus([card('Working', null)])).toBe('working');
    expect(restStatus([card('Idle', null)])).toBe('idle');
  });

  it('is total over any list of cards', () => {
    const statuses = fc.constantFrom<SessionStatus>('Working', 'Idle', 'Ended');
    fc.assert(
      fc.property(fc.array(fc.tuple(statuses, fc.boolean())), fc.boolean(), (list, prompt) => {
        const value = restStatus(
          list.map(([status, compact], index) =>
            card(status, compact ? running : null, `s${index}`),
          ),
          prompt,
        );
        expect(['idle', 'working', 'waiting', 'compacting']).toContain(value);
      }),
    );
  });
});

describe('RestMark on a compact', () => {
  it('names the state, so the colour is not the only signal', () => {
    const { container } = render(RestMark, {
      props: { status: 'compacting' as const, pct: 12, onopen: () => {} },
    });

    expect(container.querySelector('.mark')?.getAttribute('data-status')).toBe('compacting');
    expect(screen.getByRole('button').getAttribute('aria-label')).toContain('compacting');
  });

  /// The two strokes stay on screen through a compact rather than giving way
  /// to the spinner: the sign that changes colour has to be there to change.
  it('keeps the strokes rather than the working spinner', () => {
    const { container } = render(RestMark, {
      props: { status: 'compacting' as const, pct: 12, onopen: () => {} },
    });

    expect(container.querySelectorAll('.stroke')).toHaveLength(2);
  });

  /// The colour says what is happening; the hop says it just changed. An
  /// island that hops on its first paint hops for nothing. tech.md 6.21.
  it('hops when the state changes under it, and never on the first paint', async () => {
    const { container, rerender } = render(RestMark, {
      props: { status: 'idle' as const, pct: 12, onopen: () => {} },
    });
    const mark = container.querySelector('.mark');

    expect(mark).not.toHaveClass('turned');

    await rerender({ status: 'compacting' as const, pct: 12, onopen: () => {} });
    expect(mark).toHaveClass('turned');
  });
});

describe('the line in the dialogue', () => {
  it('says what is happening and how long it has been happening', () => {
    render(WorkLine, {
      props: {
        running: true,
        tone: 'compact' as const,
        words: ['Compacting'],
        from: Date.now() - 134_000,
      },
    });

    expect(screen.getByText('2m 14s')).toBeInTheDocument();
  });

  /// One pause, one colour, wherever it is drawn. tech.md 6.21.
  it('wears the same tone the sign wears', () => {
    const { container } = render(WorkLine, {
      props: { running: true, tone: 'compact' as const, words: ['Compacting'], from: 0 },
    });

    expect(container.querySelector('.work')).toHaveClass('compact');
  });

  /// The other one-word line: while a stop request stands, the work line says
  /// what is true -- that it was asked -- rather than cycling through invented
  /// words. Not `Stopping`: the agent may finish its call first, or ignore the
  /// request. tech.md 6.5.
  it('says one word about a stop that was asked for', async () => {
    render(WorkLine, {
      props: { running: true, words: [ASKED_TO_STOP], from: Date.now() - 12_000 },
    });

    expect(screen.getByText('12s')).toBeInTheDocument();
    // The line types rather than blinks, so the word arrives a letter at a
    // time -- and it arrives whole, rather than cycling on to another.
    await screen.findByText(ASKED_TO_STOP, {}, { timeout: 4000 });
  });

  it('is an ordinary work line without the tone', () => {
    const { container } = render(WorkLine, { props: { running: true, from: 0 } });

    expect(container.querySelector('.work')).not.toHaveClass('compact');
  });

  /// The feed's own working line stands down while a compact runs: the agent
  /// is not working, the CLI is, and two lines about one pause are two
  /// answers to one question. This is the argument the route passes, so what
  /// is checked here is that `false` closes the run rather than leaving a
  /// clock running beside the compacting line. tech.md 6.21.
  it('leaves no second clock ticking beside it', () => {
    const at = 1_000;
    const entries: FeedEntry[] = [
      { id: 'u1', kind: 'User', text: 'go', tool: null, detail: null, state: 'Ok', at },
      {
        id: 't1',
        kind: 'Tool',
        text: 'Read tech.md',
        tool: 'Read',
        detail: null,
        state: 'Ok',
        at: at + 1_000,
      },
    ];

    const rows = feedRows(entries, false);

    const work = rows.filter((row) => row.kind === 'work');
    expect(work).toHaveLength(1);
    expect(work[0].kind === 'work' && work[0].to).not.toBeNull();
  });
});
