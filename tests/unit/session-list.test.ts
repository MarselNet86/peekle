/**
 * S14. The list is a picker now: it is searched, it says how old a session is,
 * and rows can be renamed and put away.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { ageLabel } from '$lib/logic/age';
import {
  BUSY_ELSEWHERE,
  canContinue,
  classifyContinueOutcome,
  isBusyElsewhere,
  replyReachable,
  searchSessions,
  stopAvailable,
} from '$lib/logic/sessions';
import type { SessionCard } from '$lib/types/generated/SessionCard';

const card = (title: string, project = 'peekle'): SessionCard => ({
  session: { session_id: title, cwd: `/x/${project}`, project, pid: null, tty: null },
  title,
  status: 'Idle',
  origin: 'Observed',
  entries: [],
  agent: null,
  mode: null,
  thinking: null,
  updated_at: 0,
});

/**
 * S22. `Stop` stands under the field only while there is a turn to stop and
 * somewhere to send the stop: a working session, no open request, and a
 * field that is reachable. tech.md 6.5.
 */
describe('the stop button', () => {
  const statuses = ['Working', 'Idle', 'Ended', undefined] as const;

  it('stands only over a working session with a reachable field and no open request', () => {
    fc.assert(
      fc.property(
        fc.constantFrom(...statuses),
        fc.boolean(),
        fc.boolean(),
        fc.boolean(),
        (status, hasPrompt, owned, canContinue) => {
          const shown = stopAvailable({ status, hasPrompt, owned, canContinue });
          expect(shown).toBe(status === 'Working' && !hasPrompt && (owned || canContinue));
        },
      ),
    );
  });

  it('is offered for an observed chat that is working, since its inbox takes the request', () => {
    expect(
      stopAvailable({ status: 'Working', hasPrompt: false, owned: false, canContinue: true }),
    ).toBe(true);
  });

  it('is not offered over Allow and Deny', () => {
    expect(
      stopAvailable({ status: 'Working', hasPrompt: true, owned: true, canContinue: false }),
    ).toBe(false);
  });
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
    mode: null,
    thinking: null,
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

/** The fork is a real round trip -- spawning a process -- and a second press
 * that lands before the first one does used to start a second one racing it,
 * because nothing on screen said the first press had been taken. tech.md 6.5. */
describe('the field while a fork is in flight', () => {
  const base = { hasPrompt: false, owned: false, canContinue: true, continuing: false };

  it('takes a press on an observed chat, same as before', () => {
    expect(replyReachable(base)).toBe(true);
  });

  it('refuses a second press once the first fork is in flight', () => {
    expect(replyReachable({ ...base, continuing: true })).toBe(false);
  });

  /** Once the fork lands the card turns Owned, and from then on the field
   * answers to that instead -- a fire-and-forget write needs no guard. */
  it('reopens the moment the session is owned, in-flight or not', () => {
    expect(replyReachable({ ...base, owned: true, continuing: true })).toBe(true);
  });

  it('answering a permission request is never gated by a fork elsewhere', () => {
    expect(replyReachable({ ...base, hasPrompt: true, canContinue: false, continuing: true })).toBe(
      true,
    );
  });
});

/**
 * Whether the chat is busy elsewhere is not this attempt's failure to
 * report: continue_session refuses it for as long as another client is
 * actually driving the chat, a fact about the world rather than about one
 * attempt, so it has to be told apart from every other refusal. tech.md 6.5.
 */
describe('reading what one attempt at continuing a chat came back with', () => {
  it('is a real session to open', () => {
    expect(classifyContinueOutcome({ session_id: 's1' }, undefined)).toEqual({
      ok: true,
      sessionId: 's1',
    });
  });

  it('is busy elsewhere, worth trying again, not an error', () => {
    expect(classifyContinueOutcome(undefined, BUSY_ELSEWHERE)).toEqual({ ok: false, busy: true });
    expect(isBusyElsewhere(BUSY_ELSEWHERE)).toBe(true);
  });

  it('is a real error otherwise', () => {
    expect(classifyContinueOutcome(undefined, 'That session is gone')).toEqual({
      ok: false,
      busy: false,
      error: 'That session is gone',
    });
    expect(isBusyElsewhere('That session is gone')).toBe(false);
  });

  /** No Tauri to answer (dev, Playwright) is not a real session and not a
   * real error either -- every other command in the island route stays
   * quiet there too, rather than reporting on a backend nothing expected. */
  it('says nothing at all when there was no backend to ask', () => {
    expect(classifyContinueOutcome(null, undefined)).toEqual({
      ok: false,
      busy: false,
      error: null,
    });
  });
});
