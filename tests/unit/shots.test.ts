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
import { looksLikeImagePaste, secondsLeft, shotLines, shotName, timeLeft } from '$lib/logic/shots';
import FeedRow from '$lib/ui/FeedRow.svelte';
import PromptInput from '$lib/ui/PromptInput.svelte';
import ShotChip from '$lib/ui/ShotChip.svelte';
import ShotPreview from '$lib/ui/ShotPreview.svelte';
import ShotPrompt from '$lib/ui/ShotPrompt.svelte';
import type { ShotOffer } from '$lib/types/generated/ShotOffer';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: {
      ...real.commands,
      sendMessage: vi.fn(),
      getState: vi.fn(),
      pasteShot: vi.fn(),
    },
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
  vi.mocked(commands.pasteShot).mockClear().mockResolvedValue(null);
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

/// The offer runs out in five seconds, and the only question is whether there
/// is time to reach the key. tech.md 6.13.
describe('the seconds on the offer', () => {
  const five: ShotOffer = {
    id: '01JB',
    session_id: 's',
    project: 'peekle',
    created_at: 1000,
    expires_at: 6000,
  };

  it('rounds up, so a part of a second still reads as one', () => {
    expect(secondsLeft(five, 1000)).toBe(5);
    expect(secondsLeft(five, 1001)).toBe(5);
    expect(secondsLeft(five, 5999)).toBe(1);
  });

  it('never goes below zero, whatever the clock says', () => {
    expect(secondsLeft(five, 6000)).toBe(0);
    expect(secondsLeft(five, 60000)).toBe(0);
  });

  it('says nothing on the pill once it is out', () => {
    render(ShotPrompt, { props: { project: 'peekle', left: 0, secs: 0 } });
    expect(screen.queryByText('0s')).not.toBeInTheDocument();
  });

  it('names the seconds beside the key', () => {
    render(ShotPrompt, { props: { project: 'peekle', left: 0.6, secs: 3 } });
    expect(screen.getByText('3s')).toBeInTheDocument();
  });
});

describe('the shot at full size', () => {
  /// Forty pixels say which shot it is, not what is on it. tech.md 6.13.
  it('opens from the chip', async () => {
    const onopen = vi.fn();
    render(ShotChip, { props: { name: '01JB.png', src: 'asset://x.png', onopen } });

    await userEvent.click(screen.getByRole('button', { name: 'Open 01JB.png' }));
    expect(onopen).toHaveBeenCalledOnce();
  });

  it('closes on a click anywhere over it', async () => {
    const onclose = vi.fn();
    render(ShotPreview, { props: { name: '01JB.png', src: 'asset://x.png', onclose } });

    await userEvent.click(screen.getByRole('img', { name: '01JB.png' }));
    expect(onclose).toHaveBeenCalledOnce();
  });

  /// A picture filling the island does not look pressable; a cross does.
  it('closes on the cross', async () => {
    const onclose = vi.fn();
    render(ShotPreview, { props: { name: '01JB.png', src: 'asset://x.png', onclose } });

    await userEvent.click(screen.getByRole('button', { name: 'Close 01JB.png' }));
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('closes on Escape', async () => {
    const onclose = vi.fn();
    render(ShotPreview, { props: { name: '01JB.png', src: 'asset://x.png', onclose } });

    await userEvent.keyboard('{Escape}');
    expect(onclose).toHaveBeenCalledOnce();
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

  /// The name is a ulid and says nothing about what was captured, so the shot
  /// itself is what stands over the field. tech.md 6.13.
  it('shows the shot rather than the name of its file', () => {
    render(ShotChip, { props: { name: '01JB.png', src: 'asset://localhost/01JB.png' } });

    expect(screen.getByRole('img', { name: '01JB.png' })).toHaveAttribute(
      'src',
      'asset://localhost/01JB.png',
    );
    expect(screen.queryByText('01JB.png')).not.toBeInTheDocument();
  });

  /// The cache is a directory the user may empty at any moment. A shot that
  /// cannot be drawn still has to be visible and still has to be removable.
  it('falls back to the name when the picture will not load', async () => {
    render(ShotChip, { props: { name: '01JB.png', src: 'asset://localhost/gone.png' } });

    const picture = screen.getByRole('img', { name: '01JB.png' });
    picture.dispatchEvent(new Event('error'));
    await Promise.resolve();

    expect(screen.getByText('01JB.png')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove 01JB.png' })).toBeInTheDocument();
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
      usage: {
        windows: [],
        source: 'Fake',
        reason: null,
        fetched_at: 0,
        keychain_granted: true,
        retry_after_ms: null,
      },
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

describe('pasting a picture into the field', () => {
  /** A paste event carrying the given clipboard types. */
  function pasteOf(types: string[]): ClipboardEvent {
    const event = new Event('paste', { bubbles: true, cancelable: true }) as ClipboardEvent;
    Object.defineProperty(event, 'clipboardData', { value: { types } });
    return event;
  }

  /// Text pastes the way it does everywhere: the webview does it and the
  /// field does not interfere. Only a picture is taken over, because it is
  /// the one thing a text box cannot hold. tech.md 6.13.
  it('tells a picture from text before anything reads the clipboard', () => {
    expect(looksLikeImagePaste(['image/png'])).toBe(true);
    expect(looksLikeImagePaste(['Files'])).toBe(true);
    expect(looksLikeImagePaste(['text/plain'])).toBe(false);
    // A screenshot copied out of a browser carries both, and the words are
    // what the person meant to paste.
    expect(looksLikeImagePaste(['text/html', 'image/png'])).toBe(false);
    expect(looksLikeImagePaste([])).toBe(false);
  });

  it('takes over a picture and leaves text alone', async () => {
    const onpasteimage = vi.fn();
    const { container } = render(PromptInput, { value: '', onpasteimage });
    const field = container.querySelector('textarea') as HTMLTextAreaElement;

    const words = pasteOf(['text/plain']);
    field.dispatchEvent(words);
    expect(onpasteimage).not.toHaveBeenCalled();
    expect(words.defaultPrevented).toBe(false);

    const picture = pasteOf(['image/png']);
    field.dispatchEvent(picture);
    expect(onpasteimage).toHaveBeenCalledOnce();
    // Taken over, so the webview does not also paste a file name into the box.
    expect(picture.defaultPrevented).toBe(true);
  });

  /// The file arrives back the way the attach key's does, by the event, so
  /// one attachment is never added twice. tech.md 6.13.
  it('asks Rust for the file and takes the attachment from the event', async () => {
    const shots = createShots();
    const stop = await shots.start();
    vi.mocked(commands.pasteShot).mockResolvedValue('/cache/shots/01J.png');

    await shots.paste('s1');
    expect(commands.pasteShot).toHaveBeenCalledWith('s1');
    expect(shots.of('s1')).toEqual([]);

    attachHandler()({ session_id: 's1', path: '/cache/shots/01J.png' });
    expect(shots.of('s1')).toEqual(['/cache/shots/01J.png']);
    stop();
  });

  /// Nowhere to save it disturbs nothing: the reply being typed stays as it is.
  it('survives a refusal without touching the field', async () => {
    const shots = createShots();
    const stop = await shots.start();
    vi.mocked(commands.pasteShot).mockRejectedValue('Nowhere to save the screenshot');

    await expect(shots.paste('s1')).resolves.toBeUndefined();
    expect(shots.of('s1')).toEqual([]);
    stop();
  });
});

describe('a reply that carried a screenshot', () => {
  const SHOT = '/Users/dev/Library/Caches/peekle/shots/01M238H5HQEQB3GY5V1SMPPFYF.png';

  const said = (text: string) => ({
    id: 'e1',
    kind: 'User' as const,
    text,
    tool: null,
    detail: null,
    state: 'Ok' as const,
    at: 0,
  });

  /// The agent gets a path on its own line, which is right for the agent and
  /// useless to the person: a ulid says neither which shot it is nor what is
  /// on it. tech.md 6.13.
  it('is split into what it carried and what it said', () => {
    expect(shotLines(`${SHOT}\nlook at this`)).toEqual({ shots: [SHOT], said: 'look at this' });
    expect(shotLines('just words')).toEqual({ shots: [], said: 'just words' });
    // Prose that mentions a path is prose.
    expect(shotLines(`why is ${SHOT} empty?`).shots).toEqual([]);
    // Somebody else's png in somebody else's folder is not ours.
    expect(shotLines('/Users/dev/Pictures/01M238H5HQEQB3GY5V1SMPPFYF.png').shots).toEqual([]);
  });

  it('shows the picture and opens it when pressed', async () => {
    const onopenshot = vi.fn();
    render(FeedRow, {
      props: {
        entry: said(`${SHOT}\nlook at this`),
        shotSrc: (path: string) => `asset://${path}`,
        onopenshot,
      },
    });

    const picture = screen.getByRole('img');
    expect(picture).toHaveAttribute('src', `asset://${SHOT}`);
    // The words stay, and the path does not stand in them twice.
    expect(screen.getByText('look at this')).toBeInTheDocument();
    expect(screen.queryByText(SHOT)).toBeNull();

    await userEvent.click(screen.getByRole('button', { name: /Open/ }));
    expect(onopenshot).toHaveBeenCalledWith(SHOT);
  });

  /// The cache is a cache and the user may empty it. A reply must still show
  /// what actually went to the agent, so the line comes back. tech.md 6.13.
  it('falls back to the line when the picture will not load', async () => {
    const { container } = render(FeedRow, {
      props: {
        entry: said(`${SHOT}\nlook at this`),
        shotSrc: (path: string) => `asset://${path}`,
      },
    });

    const picture = container.querySelector('img') as HTMLImageElement;
    picture.dispatchEvent(new Event('error'));
    await vi.waitFor(() => expect(container.querySelector('img')).toBeNull());
    expect(screen.getByText(SHOT, { exact: false })).toBeInTheDocument();
  });

  /// Nowhere to read pictures from, so nothing pretends there are any.
  it('stays a line where no picture can be drawn', () => {
    render(FeedRow, { props: { entry: said(`${SHOT}\nlook at this`) } });

    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText(SHOT, { exact: false })).toBeInTheDocument();
  });
});
