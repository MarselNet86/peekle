/**
 * S15 acceptance. A screenshot lives on the pasteboard, and the island is what
 * carries it into a session: an offer that stands for seconds, a key that
 * answers it, and an attachment that waits beside the reply. tech.md 6.13.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import fc from 'fast-check';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { commands, events } from '$lib/bridge';
import { createIsland } from '$lib/features/island/island.svelte';
import { createShots } from '$lib/features/shots/shots.svelte';
import { shotName, timeLeft } from '$lib/logic/shots';
import ShotChip from '$lib/ui/ShotChip.svelte';
import ShotPrompt from '$lib/ui/ShotPrompt.svelte';
import type { ShotOffer } from '$lib/types/generated/ShotOffer';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: { ...real.commands, sendMessage: vi.fn(), getState: vi.fn() },
    events: { ...real.events, onShot: vi.fn(), onShotAttached: vi.fn() },
  };
});

/** Hands back the handler Rust would call, so a real event can be played. */
function attachHandler(): (payload: { session_id: string; path: string }) => void {
  const [handler] = vi.mocked(events.onShotAttached).mock.calls.at(-1) ?? [];
  if (!handler) throw new Error('nothing subscribed to the attach event');
  return handler;
}

beforeEach(() => {
  vi.mocked(commands.sendMessage).mockClear();
  vi.mocked(commands.getState).mockClear().mockResolvedValue(null);
  vi.mocked(events.onShot)
    .mockClear()
    .mockResolvedValue(() => {});
  vi.mocked(events.onShotAttached)
    .mockClear()
    .mockResolvedValue(() => {});
});

const offer = (created: number, expires: number): ShotOffer => ({
  id: '01JBQ8WMKX',
  session_id: 's1',
  project: 'peekle',
  created_at: created,
  expires_at: expires,
});

describe('the fuse on the offer', () => {
  it('runs from full to empty across the window', () => {
    const five = offer(1000, 6000);

    expect(timeLeft(five, 1000)).toBe(1);
    expect(timeLeft(five, 3500)).toBeCloseTo(0.5);
    expect(timeLeft(five, 6000)).toBe(0);
  });

  /// The bar is drawn from a clock nobody controls, and it renders every
  /// hundred milliseconds. A width outside 0..1 is a bar out of its own pill.
  it('stays inside its own pill for any clock at all', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: -1e12, max: 1e12 }),
        fc.integer({ min: -1e12, max: 1e12 }),
        fc.integer({ min: -1e12, max: 1e12 }),
        (created, expires, now) => {
          const left = timeLeft(offer(created, expires), now);
          expect(left).toBeGreaterThanOrEqual(0);
          expect(left).toBeLessThanOrEqual(1);
          expect(Number.isNaN(left)).toBe(false);
        },
      ),
    );
  });

  /// An offer already settled draws nothing rather than a negative bar. Rust
  /// closes it on the same deadline, so this is the frame in between.
  it('reads as spent once the deadline has passed', () => {
    expect(timeLeft(offer(0, 5000), 9000)).toBe(0);
    expect(timeLeft(offer(0, 0), 0)).toBe(0);
  });
});

describe('what the attachment is called', () => {
  it('is the file name, never the path the agent gets', () => {
    expect(shotName('/Users/x/Library/Caches/peekle/shots/01JB.png')).toBe('01JB.png');
    expect(shotName('01JB.png')).toBe('01JB.png');
  });

  it('is total over anything it is handed', () => {
    fc.assert(
      fc.property(fc.string(), (path) => {
        expect(typeof shotName(path)).toBe('string');
      }),
    );
  });
});

describe('the offer in the notch', () => {
  /// It has five seconds to say three things: that there is a screenshot,
  /// where it would go, and which key sends it there. tech.md 6.13.
  it('names the session it would attach to and the key that answers', () => {
    render(ShotPrompt, { props: { project: 'peekle', left: 1 } });

    expect(screen.getByText(/Screenshot to peekle/)).toBeInTheDocument();
    expect(screen.getByText('↑')).toBeInTheDocument();
  });

  it('draws the time it has left', () => {
    const { container } = render(ShotPrompt, { props: { project: 'peekle', left: 0.25 } });
    const fuse = container.querySelector('.fuse');

    expect(fuse).toBeInstanceOf(HTMLElement);
    expect((fuse as HTMLElement).style.getPropertyValue('--left')).toBe('0.25');
  });
});

describe('the attachment beside the field', () => {
  it('gives the shot back when the cross is pressed', async () => {
    const onremove = vi.fn();
    render(ShotChip, { props: { name: '01JB.png', onremove } });

    await userEvent.click(screen.getByRole('button', { name: 'Remove 01JB.png' }));
    expect(onremove).toHaveBeenCalledOnce();
  });

  /// The caret is in the reply box beside it, and taking an attachment back
  /// must not pull the user out of the sentence they are typing. tech.md 6.7.
  it('does not take focus away from the field', async () => {
    render(ShotChip, { props: { name: '01JB.png' } });
    const cross = screen.getByRole('button', { name: 'Remove 01JB.png' });

    await userEvent.click(cross);
    expect(document.activeElement).not.toBe(cross);
  });
});

describe('the shot waiting in the field', () => {
  /// Attaching is not sending. The shot sits by the field until the user says
  /// what they want done with it. tech.md 6.13.
  it('waits in the session it was attached to and in no other', async () => {
    const shots = createShots();
    const stop = await shots.start();

    attachHandler()({ session_id: 's1', path: '/cache/01JB.png' });

    expect(shots.of('s1')).toEqual(['/cache/01JB.png']);
    expect(shots.of('s2')).toEqual([]);
    stop();
  });

  it('takes one back off the message and leaves the rest', async () => {
    const shots = createShots();
    const stop = await shots.start();

    attachHandler()({ session_id: 's1', path: '/cache/a.png' });
    attachHandler()({ session_id: 's1', path: '/cache/b.png' });
    shots.remove('s1', '/cache/a.png');

    expect(shots.of('s1')).toEqual(['/cache/b.png']);
    stop();
  });

  it('is empty again once the message has gone', async () => {
    const shots = createShots();
    const stop = await shots.start();

    attachHandler()({ session_id: 's1', path: '/cache/a.png' });
    shots.clear('s1');

    expect(shots.of('s1')).toEqual([]);
    stop();
  });

  /// The panel is put up before the route mounts, so an offer can already be
  /// standing by the time anything here subscribes.
  it('picks up an offer that was already standing when it mounted', async () => {
    vi.mocked(commands.getState).mockResolvedValue({
      enabled: true,
      view: 'Pill',
      active_prompt: null,
      sessions: [],
      tasks: [],
      usage: { windows: [], source: 'Fake', reason: null, fetched_at: 0 },
      shot: offer(0, 5000),
      live_sessions: 0,
      hotkey_ok: true,
    });

    const shots = createShots();
    const stop = await shots.start();

    expect(shots.offer?.project).toBe('peekle');
    stop();
  });
});

describe('sending an attached screenshot', () => {
  /// The paths travel with the message and Rust composes the wire text: what
  /// goes into the pty is a delivery detail, not a decision for the window.
  /// tech.md 6.13.
  it('hands the paths to the session along with the text', () => {
    const island = createIsland('');
    island.answer('what is wrong here', 's1', ['/cache/a.png']);

    expect(commands.sendMessage).toHaveBeenCalledExactlyOnceWith('s1', 'what is wrong here', [
      '/cache/a.png',
    ]);
  });

  it('sends no attachment when none is waiting', () => {
    const island = createIsland('');
    island.answer('carry on', 's1');

    expect(commands.sendMessage).toHaveBeenCalledExactlyOnceWith('s1', 'carry on', []);
  });
});
