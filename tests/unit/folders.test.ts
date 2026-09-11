/**
 * The folder a new chat works in. tech.md 6.23.
 *
 * Acceptance, from the change: a chat that has not begun can be pointed at any
 * project the island knows, each named once and newest first, with a way out to
 * the rest of the disk last; a chat that has begun cannot be pointed anywhere,
 * because the agent is already living in a folder.
 */

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
  canPickFolder,
  CHOOSE,
  CHOOSE_LABEL,
  folderOptions,
  knownFolders,
} from '$lib/logic/folders';
import type { FeedEntry } from '$lib/types/generated/FeedEntry';
import type { SessionCard } from '$lib/types/generated/SessionCard';

function card(id: string, cwd: string, over: Partial<SessionCard> = {}): SessionCard {
  const project = cwd.split('/').filter(Boolean).at(-1) ?? '';
  return {
    session: { session_id: id, cwd, project, pid: null, tty: null },
    title: '',
    status: 'Idle',
    origin: 'Owned',
    entries: [],
    agent: null,
    mode: null,
    thinking: null,
    compacting: null,
    stopping: null,
    asking_trust: null,
    updated_at: 0,
    ...over,
  } as SessionCard;
}

const said: FeedEntry = {
  id: 'u1',
  kind: 'User',
  text: 'go on',
  tool: null,
  detail: null,
  state: 'Ok',
  at: 1_789_000_000_000,
};

describe('the folders the island knows', () => {
  /// Nothing is scanned for this list: it is the chats already on screen, so
  /// it is exactly the projects Claude Code has been run in.
  it('names every folder once, in the order the chats came', () => {
    const folders = knownFolders([
      card('s1', '/Users/dev/peekle'),
      card('s2', '/Users/dev/site'),
      card('s3', '/Users/dev/peekle'),
    ]);

    expect(folders).toEqual([
      { cwd: '/Users/dev/peekle', project: 'peekle' },
      { cwd: '/Users/dev/site', project: 'site' },
    ]);
  });

  it('has nothing to offer when the island knows no chats', () => {
    expect(knownFolders([])).toEqual([]);
  });

  /// A folder is a path, and the way out is not: the row that opens the macOS
  /// dialog travels as an id beside real paths, so it has to be something no
  /// path can be.
  it('keeps the way out apart from every path', () => {
    fc.assert(
      fc.property(fc.array(fc.string(), { minLength: 0, maxLength: 6 }), (names) => {
        const cards = names.map((name, index) => card(`s${index}`, `/Users/dev/${name}`));
        const options = folderOptions(cards, '');
        const rows = options.filter((option) => option.id === CHOOSE);
        expect(rows).toHaveLength(1);
        expect(options.at(-1)?.id).toBe(CHOOSE);
        expect(options.at(-1)?.label).toBe(CHOOSE_LABEL);
      }),
    );
  });

  /// The full path under the name: two chats can be in two folders called
  /// `site`, and a menu that shows only the name cannot tell them apart.
  it('says where each folder is, under its name', () => {
    const [first] = folderOptions([card('s1', '/Users/dev/peekle')], '/Users/dev/peekle');

    expect(first.label).toBe('peekle');
    expect(first.hint).toBe('/Users/dev/peekle');
    expect(first.id).toBe('/Users/dev/peekle');
  });

  /// A folder just taken from the dialog belongs to no chat yet, and the tick
  /// has to have a row to stand on.
  it('carries a folder no chat has yet, so the tick has a row', () => {
    const options = folderOptions([card('s1', '/Users/dev/peekle')], '/Users/dev/fresh');

    expect(options[0].id).toBe('/Users/dev/fresh');
    expect(options[0].label).toBe('fresh');
    expect(options.map((option) => option.id)).toContain('/Users/dev/peekle');
  });
});

describe('which chats can still choose', () => {
  it('lets a chat of ours that has said nothing choose', () => {
    expect(canPickFolder(card('s1', '/Users/dev/peekle'))).toBe(true);
  });

  /// The process was started in that folder and no `cd` reaches a running
  /// agent, so the moment something is said the choice is over.
  it('does not offer the choice once something has been said', () => {
    expect(canPickFolder(card('s1', '/Users/dev/peekle', { entries: [said] }))).toBe(false);
  });

  /// A chat somebody else runs has no folder of ours to move.
  it('does not offer it on a chat the island only reads', () => {
    expect(canPickFolder(card('s1', '/Users/dev/peekle', { origin: 'Observed' }))).toBe(false);
  });

  /// And a chat whose process is gone is not choosing anything either.
  it('does not offer it on a chat that has ended', () => {
    expect(canPickFolder(card('s1', '/Users/dev/peekle', { status: 'Ended' }))).toBe(false);
  });

  it('has nothing to offer when there is no chat at all', () => {
    expect(canPickFolder(undefined)).toBe(false);
  });
});
