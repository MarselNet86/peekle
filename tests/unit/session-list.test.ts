/**
 * S14. The list is a picker now: it is searched, it says how old a session is,
 * and rows can be renamed and put away.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { ageLabel } from '$lib/logic/age';
import { canContinue, searchSessions } from '$lib/logic/sessions';
import type { SessionCard } from '$lib/types/generated/SessionCard';

const card = (title: string, project = 'peekle'): SessionCard => ({
  session: { session_id: title, cwd: `/x/${project}`, project, pid: null, tty: null },
  title,
  status: 'Idle',
  origin: 'Observed',
  entries: [],
  agent: null,
  updated_at: 0,
});

describe('the age of a session', () => {
  it('writes one unit, the largest that is true', () => {
    const now = 10 * 24 * 3600_000;
    expect(ageLabel(now - 30_000, now)).toBe('now');
    expect(ageLabel(now - 50 * 60_000, now)).toBe('50m');
    expect(ageLabel(now - 3 * 3600_000, now)).toBe('3h');
    expect(ageLabel(now - 4 * 24 * 3600_000, now)).toBe('4d');
  });

  /// A garbage timestamp must not push the title out of its own row.
  it('caps an age nobody would read anyway', () => {
    const year = 365 * 24 * 3600_000;
    expect(ageLabel(0, 2 * year)).toBe('2y');
    expect(ageLabel(0, 500 * year)).toBe('99y+');
  });

  /// Clocks disagree, and no row may ever read `-2m`.
  it('reads a timestamp from the future as now', () => {
    expect(ageLabel(2000, 1000)).toBe('now');
  });

  it('is total and stays short enough to sit next to a title', () => {
    fc.assert(
      fc.property(fc.double({ noNaN: false }), fc.double({ noNaN: false }), (at, now) => {
        const label = ageLabel(at, now);
        expect(typeof label).toBe('string');
        expect(label.length).toBeLessThanOrEqual(5);
      }),
    );
  });
});

describe('searching the list', () => {
  const cards = [card('Fix the panel'), card('Ship it', 'uplink'), card('фикс окна')];

  it('matches the title and the project, whatever the case', () => {
    expect(searchSessions(cards, 'PANEL').map((c) => c.title)).toEqual(['Fix the panel']);
    expect(searchSessions(cards, 'uplink').map((c) => c.title)).toEqual(['Ship it']);
    expect(searchSessions(cards, 'ФИКС').map((c) => c.title)).toEqual(['фикс окна']);
  });

  it('keeps everything when nothing was asked', () => {
    expect(searchSessions(cards, '')).toEqual(cards);
    expect(searchSessions(cards, '   ')).toEqual(cards);
  });

  it('never invents a row and never reorders one', () => {
    fc.assert(
      fc.property(fc.string(), (query) => {
        const kept = searchSessions(cards, query);
        expect(kept.length).toBeLessThanOrEqual(cards.length);
        expect(kept).toEqual(cards.filter((c) => kept.includes(c)));
      }),
    );
  });
});

describe('continuing a chat', () => {
  const card = (origin: SessionCard['origin']): SessionCard => ({
    session: { session_id: 's', cwd: '/x/p', project: 'p', pid: null, tty: null },
    title: 't',
    status: 'Idle',
    origin,
    entries: [],
    agent: null,
    updated_at: 0,
  });

  /** An observed chat has no field, but it can be forked into one we own the
   * way Desktop opens an existing chat. An owned one already has a field, and
   * nothing to continue means nothing to offer. tech.md 6.5. */
  it('offers the fork only for an observed chat', () => {
    expect(canContinue(card('Observed'))).toBe(true);
    expect(canContinue(card('Owned'))).toBe(false);
    expect(canContinue(undefined)).toBe(false);
  });
});
