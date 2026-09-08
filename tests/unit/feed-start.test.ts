/**
 * The island shows what Rust knows, not only what it happened to hear. An
 * event sent before the subscription landed reaches nobody, and the backfill
 * sends one at start, so the list is asked for once on mount. tech.md 8.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { commands, events } from '$lib/bridge';
import { createFeed } from '$lib/features/feed/feed.svelte';
import type { SessionCard } from '$lib/types/generated/SessionCard';

const card = (id: string): SessionCard =>
  ({
    session: { session_id: id, cwd: '/Users/mars/peekle', project: 'peekle', pid: null, tty: null },
    title: id,
    status: 'Idle',
    origin: 'Observed',
    entries: [],
    updated_at: 1,
    agent: null,
  }) as unknown as SessionCard;

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: { ...real.commands, getSessions: vi.fn() },
    events: { ...real.events, onSessions: vi.fn() },
  };
});

beforeEach(() => {
  vi.mocked(commands.getSessions).mockReset();
  vi.mocked(events.onSessions).mockReset();
});

describe('the list on mount', () => {
  /// The bug this closes: transcripts on disk, the server answering, and the
  /// island saying "No sessions yet" because the one event it lives on was
  /// sent before it was listening.
  it('asks for the cards rather than waiting to be told', async () => {
    vi.mocked(events.onSessions).mockResolvedValue(() => {});
    vi.mocked(commands.getSessions).mockResolvedValue([card('s-1'), card('s-2')]);

    const feed = createFeed();
    await feed.start();

    expect(feed.sessions.map((c) => c.session.session_id)).toEqual(['s-1', 's-2']);
  });

  /// Subscribing first is the point: a push that lands while the snapshot is
  /// in flight is newer than the snapshot by definition.
  it('keeps a push that arrived while it was asking', async () => {
    vi.mocked(events.onSessions).mockImplementation(async (handler) => {
      handler([card('pushed')]);
      return () => {};
    });
    vi.mocked(commands.getSessions).mockResolvedValue([card('older')]);

    const feed = createFeed();
    await feed.start();

    expect(feed.sessions.map((c) => c.session.session_id)).toEqual(['pushed']);
  });

  /// Outside the app shell there is no Tauri and the command answers nothing.
  /// An island that renders on its own is the rule every route lives by.
  it('renders empty rather than breaking when nobody answers', async () => {
    vi.mocked(events.onSessions).mockResolvedValue(() => {});
    vi.mocked(commands.getSessions).mockResolvedValue(undefined as unknown as SessionCard[]);

    const feed = createFeed();
    await feed.start();

    expect(feed.sessions).toEqual([]);
  });
});
