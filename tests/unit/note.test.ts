/**
 * The answer to a press that cannot do what was asked: it stands for a term
 * the reader can see, it can be closed, and reading it does not spend it.
 * tech.md 9 and 6.15.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ELSEWHERE_NOTE, NOTE_EXIT_MS, NOTE_TTL } from '$lib/logic/agent';
import ContextRing from '$lib/ui/ContextRing.svelte';
import NoteBlock from '$lib/ui/NoteBlock.svelte';

describe('the note that answers a press', () => {
  it('says the fact and the way out', () => {
    render(NoteBlock, { props: { fact: ELSEWHERE_NOTE.fact, how: ELSEWHERE_NOTE.how } });

    expect(screen.getByText(ELSEWHERE_NOTE.fact)).toBeInTheDocument();
    expect(screen.getByText(ELSEWHERE_NOTE.how!)).toBeInTheDocument();
  });

  /// A block with no way out sits over the conversation until the session
  /// changes. The cross is the answer to "I have read it".
  it('closes when the cross is pressed', async () => {
    const onclose = vi.fn();
    render(NoteBlock, { props: { fact: 'no', onclose } });

    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    await vi.waitFor(() => expect(onclose).toHaveBeenCalledOnce());
  });

  /// The term is shown, not guessed at: a hairline under the text carries it.
  it('carries its term as a hairline', () => {
    const { container } = render(NoteBlock, { props: { fact: 'no' } });

    const note = container.querySelector('.note') as HTMLElement;
    expect(note.style.getPropertyValue('--secs')).toBe(`${NOTE_TTL / 1000}s`);
    expect(container.querySelector('.leak')).not.toBeNull();
  });

  /// Nothing leaks when nothing is running out, so no hairline either.
  it('draws no hairline when it stands until closed', () => {
    const { container } = render(NoteBlock, { props: { fact: 'no', ttl: 0 } });

    expect(container.querySelector('.leak')).toBeNull();
  });
});

describe('the term of a note', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('is ten seconds', () => {
    expect(NOTE_TTL).toBe(10_000);
  });

  it('goes when the term is up, and not before', async () => {
    const onclose = vi.fn();
    render(NoteBlock, { props: { fact: 'no', onclose } });

    await vi.advanceTimersByTimeAsync(NOTE_TTL - 500);
    expect(onclose).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(500 + NOTE_EXIT_MS);
    expect(onclose).toHaveBeenCalledOnce();
  });

  /// A person who is reading it is not spending it.
  it('holds while the pointer is on it', async () => {
    const onclose = vi.fn();
    const { container } = render(NoteBlock, { props: { fact: 'no', onclose } });
    const note = container.querySelector('.note') as HTMLElement;

    await vi.advanceTimersByTimeAsync(2_000);
    note.dispatchEvent(new MouseEvent('mouseenter'));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(onclose).not.toHaveBeenCalled();

    // Leaving picks the term up where it was left, not at the beginning.
    note.dispatchEvent(new MouseEvent('mouseleave'));
    await vi.advanceTimersByTimeAsync(NOTE_TTL - 2_000 + NOTE_EXIT_MS);
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('never closes on its own when it has no term', async () => {
    const onclose = vi.fn();
    render(NoteBlock, { props: { fact: 'no', ttl: 0, onclose } });

    await vi.advanceTimersByTimeAsync(60_000);
    expect(onclose).not.toHaveBeenCalled();
  });
});

describe('the compact ring on a chat the island does not run', () => {
  /// It used to be disabled, so a press did nothing at all: no compact and no
  /// explanation either. tech.md 6.15.
  it('takes the press and answers it', async () => {
    const onclick = vi.fn();
    render(ContextRing, { props: { pct: 56, title: 'ring', live: false, onclick } });

    const ring = screen.getByRole('button', { name: 'ring' });
    expect(ring).toBeEnabled();
    await userEvent.click(ring);
    expect(onclick).toHaveBeenCalledOnce();
  });

  /// Nothing has been asked of the window yet, so there is nothing to compact
  /// and nothing to explain.
  it('stays dead while there is no number', () => {
    render(ContextRing, { props: { pct: null, title: 'ring', live: false } });

    expect(screen.getByRole('button', { name: 'ring' })).toBeDisabled();
  });
});

describe('the compact ring itself', () => {
  /// The grey circle under the arc reports nothing where the ring is a
  /// button, and at the edge of the row it read brighter than the arc.
  /// tech.md 9.
  it('draws no track under the arc', () => {
    const { container } = render(ContextRing, {
      props: { pct: 56, title: 'ring', live: true },
    });

    expect(container.querySelector('.fill')).not.toBeNull();
    expect(container.querySelector('.track')).toBeNull();
  });

  /// With no arc and no track there is nothing on screen, and a button nobody
  /// can see is a button nobody presses.
  it('keeps the track while there is no number', () => {
    const { container } = render(ContextRing, {
      props: { pct: null, title: 'ring', live: true },
    });

    expect(container.querySelector('.track')).not.toBeNull();
  });
});
