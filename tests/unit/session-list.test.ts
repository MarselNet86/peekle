/**
 * S14. The list is a picker now: it is searched, it says how old a session is,
 * and rows can be renamed and put away.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { ageLabel } from '$lib/logic/age';
import {
  canContinue,
  classifyContinueOutcome,
  replyReachable,
  searchSessions,
  steadyOrder,
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
  compacting: null,
  stopping: null,
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

  /// A stop into a chat Peekle does not own is a message, and a second press
  /// is a second message and a second turn in somebody's chat. One press,
  /// then wait. tech.md 6.5.
  it('takes one press while the request it sent is still standing', () => {
    const working = {
      status: 'Working',
      hasPrompt: false,
      owned: false,
      canContinue: true,
    } as const;

    expect(stopAvailable({ ...working, asked: null })).toBe(true);
    expect(stopAvailable({ ...working, asked: 1_789_000_000_000 })).toBe(false);
  });

  it('never asks twice, whatever else is true', () => {
    fc.assert(
      fc.property(
        fc.constantFrom(...statuses),
        fc.boolean(),
        fc.boolean(),
        fc.boolean(),
        (status, hasPrompt, owned, canContinue) => {
          expect(stopAvailable({ status, hasPrompt, owned, canContinue, asked: 1 })).toBe(false);
        },
      ),
    );
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
    compacting: null,
    stopping: null,
    updated_at: 0,
  });

  /** Every chat the island knows takes words since v66, whoever started it
   * and whether or not its process is still up: one nobody holds is resumed,
   * one a live process holds goes through that process, one that is held and
   * takes nothing is copied. Which of the three is Rust's call at the instant
   * of sending. Only a chat that does not exist has nothing to offer.
   * tech.md 6.5. */
  it('takes words for every chat it knows, whoever started it', () => {
    expect(canContinue(card('Observed'))).toBe(true);
    expect(canContinue(card('Owned'))).toBe(true);
    expect(canContinue({ ...card('Owned'), status: 'Ended' })).toBe(true);
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
 * v66 acceptance. Two outcomes, not three: the words reached a chat or they
 * did not. Nothing refuses any more -- a chat held by another app that takes
 * no messages is copied rather than declined -- so "busy elsewhere" has no
 * caller left. tech.md 6.5.
 */
describe('reading what one attempt at continuing a chat came back with', () => {
  it('is a real session to open', () => {
    expect(classifyContinueOutcome({ session_id: 's1' }, undefined)).toEqual({
      ok: true,
      sessionId: 's1',
    });
  });

  /** The id that comes back is where the words actually landed. A different
   * one means the chat was held and this is the copy of it. */
  it('names the chat the words landed in, copy or not', () => {
    expect(classifyContinueOutcome({ session_id: 'copy-of-s1' }, undefined)).toEqual({
      ok: true,
      sessionId: 'copy-of-s1',
    });
  });

  it('is a real error otherwise', () => {
    expect(classifyContinueOutcome(undefined, 'That session is gone')).toEqual({
      ok: false,
      error: 'That session is gone',
    });
  });

  /** No Tauri to answer (dev, Playwright) is not a real session and not a
   * real error either -- every other command in the island route stays
   * quiet there too, rather than reporting on a backend nothing expected. */
  it('says nothing at all when there was no backend to ask', () => {
    expect(classifyContinueOutcome(null, undefined)).toEqual({ ok: false, error: null });
  });
});

describe('the order the list is read in', () => {
  const ids = (cards: SessionCard[]) => cards.map((each) => each.session.session_id);

  /// Rust sorts by activity, which is right for one chat and wrong for
  /// several: a turn in a chat nobody is watching lifted it to the top and
  /// pushed every row under it down, so a press landed on whichever row had
  /// taken the place of the one aimed at. tech.md 6.12.
  it('keeps rows where they were while the list is open', () => {
    const opened = ['a', 'b', 'c'];
    // 'c' just answered, so the data now leads with it.
    const now = [card('c'), card('a'), card('b')];

    expect(ids(steadyOrder(now, opened))).toEqual(['a', 'b', 'c']);
  });

  /// A chat the list has never shown is new to the reader too, and the top is
  /// where a new chat belongs.
  it('puts a chat it has not seen at the top', () => {
    const opened = ['a', 'b'];
    const now = [card('fresh'), card('b'), card('a')];

    expect(ids(steadyOrder(now, opened))).toEqual(['fresh', 'a', 'b']);
  });

  /// Nothing held yet, so nothing to hold to: the data's own order stands.
  it('takes the data as it comes when nothing is held', () => {
    const now = [card('c'), card('a')];

    expect(ids(steadyOrder(now, []))).toEqual(['c', 'a']);
  });

  /// A chat that went away leaves no hole and takes nothing with it.
  it('survives a chat that is gone', () => {
    const opened = ['a', 'gone', 'b'];
    const now = [card('b'), card('a')];

    expect(ids(steadyOrder(now, opened))).toEqual(['a', 'b']);
  });

  it('never loses or duplicates a row, whatever it is given', () => {
    fc.assert(
      fc.property(
        fc.array(fc.string({ minLength: 1, maxLength: 4 }), { maxLength: 8 }),
        fc.array(fc.string({ minLength: 1, maxLength: 4 }), { maxLength: 8 }),
        (present, opened) => {
          const unique = [...new Set(present)];
          const held = steadyOrder(
            unique.map((id) => card(id)),
            opened,
          );
          expect(ids(held).sort()).toEqual([...unique].sort());
        },
      ),
    );
  });
});
