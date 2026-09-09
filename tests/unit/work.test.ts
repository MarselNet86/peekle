/**
 * The feed folds a run of calls into one line with a clock. These check the
 * fold against what v57 asks of it — every reply kept, every call gone, the
 * run dated from the record before it — and never against how it walks the
 * list. tech.md 6.12.
 */

import { render, screen } from '@testing-library/svelte';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { TAIL_ID, elapsedLabel, feedRows, type WorkRow } from '$lib/logic/work';
import WorkLine from '$lib/ui/WorkLine.svelte';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';

const entry = (over: Partial<FeedEntry> = {}): FeedEntry => ({
  id: '01J0',
  kind: 'Tool',
  text: "cd /Users/dev; python3 - <<'PY'",
  tool: 'Bash',
  detail: null,
  state: 'Ok',
  at: 0,
  ...over,
});

const said = (id: string, at: number, kind: FeedEntry['kind'] = 'Assistant') =>
  entry({ id, at, kind, tool: null, text: `${id} words` });

const call = (id: string, at: number) => entry({ id, at });

function works(rows: ReturnType<typeof feedRows>): WorkRow[] {
  return rows.filter((row): row is WorkRow => row.kind === 'work');
}

describe('feedRows', () => {
  it('folds a run of calls between two replies into one line', () => {
    const rows = feedRows(
      [
        said('ask', 1_000, 'User'),
        call('a', 2_000),
        call('b', 3_000),
        call('c', 4_000),
        said('answer', 9_000),
      ],
      false,
    );

    expect(rows.map((row) => row.kind)).toEqual(['said', 'work', 'said']);
    expect(works(rows)[0]).toMatchObject({ from: 1_000, to: 9_000, steps: 3 });
  });

  it('shows no call and no reasoning marker of its own', () => {
    const rows = feedRows(
      [said('ask', 0, 'User'), call('a', 1), entry({ id: 't', kind: 'Thought', at: 2 })],
      false,
    );

    const kinds = rows.flatMap((row) => (row.kind === 'said' ? [row.entry.kind] : []));
    expect(kinds).toEqual(['User']);
  });

  it('dates a run from the record before it, not from its own first call', () => {
    // The person started counting when they sent, and the first long thought
    // of the turn is part of what they waited through.
    const rows = feedRows([said('ask', 1_000, 'User'), call('a', 30_000)], false);

    expect(works(rows)[0].from).toBe(1_000);
  });

  it('leaves a running run open, so the line runs its own clock', () => {
    const rows = feedRows([said('ask', 1_000, 'User'), call('a', 2_000)], true);

    expect(works(rows)[0].to).toBeNull();
  });

  it('closes a run of a session that is no longer working', () => {
    const rows = feedRows([said('ask', 1_000, 'User'), call('a', 2_000)], false);

    expect(works(rows)[0].to).toBe(2_000);
  });

  it('carries a line for a turn that has called nothing yet', () => {
    const rows = feedRows([said('ask', 1_000, 'User')], true);

    expect(rows[rows.length - 1]).toEqual({
      kind: 'work',
      id: TAIL_ID,
      from: 1_000,
      to: null,
      steps: 0,
    });
  });

  it('gives an empty working feed a line with nothing to date it', () => {
    expect(feedRows([], true)).toEqual([
      { kind: 'work', id: TAIL_ID, from: null, to: null, steps: 0 },
    ]);
  });

  it('keeps two runs apart when the agent spoke between them', () => {
    const rows = feedRows([call('a', 1), said('mid', 2), call('b', 3), said('end', 4)], false);

    expect(rows.map((row) => row.kind)).toEqual(['work', 'said', 'work', 'said']);
  });

  it('is a fold, not a filter: every reply survives in its own order', () => {
    const anyEntry = fc.record({
      id: fc.string({ minLength: 1, maxLength: 6 }),
      kind: fc.constantFrom<FeedEntry['kind']>('User', 'Assistant', 'Tool', 'Thought', 'Notice'),
      at: fc.integer({ min: 0, max: 10_000 }),
    });

    fc.assert(
      fc.property(fc.array(anyEntry, { maxLength: 40 }), fc.boolean(), (raw, working) => {
        const entries = raw.map((one, index) =>
          entry({ id: `${index}`, kind: one.kind, at: one.at }),
        );
        const rows = feedRows(entries, working);

        const spoken = entries.filter((one) => !['Tool', 'Thought'].includes(one.kind));
        const kept = rows.flatMap((row) => (row.kind === 'said' ? [row.entry] : []));
        expect(kept).toEqual(spoken);

        // Two work lines side by side would mean a run was cut in half.
        for (let at = 1; at < rows.length; at += 1) {
          expect(rows[at].kind === 'work' && rows[at - 1].kind === 'work').toBe(false);
        }

        // Nothing counts backwards, whatever the clocks in the file say.
        for (const work of works(rows)) {
          if (work.from !== null && work.to !== null) expect(work.to).toBeGreaterThanOrEqual(0);
        }
      }),
    );
  });
});

describe('elapsedLabel', () => {
  it('writes the shortest true form', () => {
    expect(elapsedLabel(9_400)).toBe('9s');
    expect(elapsedLabel(64_000)).toBe('1m 4s');
    expect(elapsedLabel(7_620_000)).toBe('2h 7m');
  });

  it('never says a run took less than nothing', () => {
    fc.assert(
      fc.property(fc.integer({ min: -100_000, max: 100_000 }), (ms) => {
        expect(elapsedLabel(ms)).not.toContain('-');
      }),
    );
  });
});

describe('WorkLine', () => {
  it('writes the clock while the agent is out', () => {
    render(WorkLine, { props: { running: true, from: Date.now() - 42_000 } });

    expect(screen.getByText('42s')).toBeInTheDocument();
  });

  it('says how long it took once the agent is back', () => {
    render(WorkLine, { props: { running: false, from: 0, to: 74_000 } });

    expect(screen.getByText('Worked for 1m 14s')).toBeInTheDocument();
  });

  it('shows the mark alone when nothing dates the run', () => {
    const { container } = render(WorkLine, { props: { running: true, from: null } });

    expect(container.querySelector('.clock')).toBeNull();
    expect(container.querySelector('.star')).not.toBeNull();
  });
});
