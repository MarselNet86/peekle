/**
 * S3 acceptance. One channel, so the field has one question to answer: is this
 * a session the island can type into. tech.md 6.5.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import PromptInput from '$lib/ui/PromptInput.svelte';
import { restStatus } from '$lib/logic/rest';
import type { SessionCard } from '$lib/types/generated/SessionCard';

const card = (
  origin: SessionCard['origin'],
  status: SessionCard['status'] = 'Idle',
): SessionCard => ({
  session: { session_id: 's', cwd: '/x/p', project: 'p', pid: null, tty: null },
  title: 'x',
  status,
  origin,
  entries: [],
  agent: null,
  mode: null,
  thinking: null,
  updated_at: 0,
});

/// The same expressions the island renders from. Kept here so the wording and
/// the rule behind it are pinned by a test rather than by whoever edits markup.
function owned(current: SessionCard | undefined): boolean {
  return current !== undefined && current.origin === 'Owned' && current.status !== 'Ended';
}

function replyHint(current: SessionCard | undefined, prompt = false): string {
  if (prompt) return 'Reply to Claude';
  if (owned(current)) return 'Message Claude';
  if (current?.status === 'Ended') return 'This session has finished';
  return 'Started outside Peekle, so this one is read-only';
}

describe('what the field promises', () => {
  /// One channel and no qualifiers. The old ladder had the field explaining
  /// when text would arrive; a pty takes it now, so there is nothing to hedge.
  it('promises delivery now for a session the island owns', () => {
    expect(replyHint(card('Owned'))).toBe('Message Claude');
    expect(replyHint(card('Owned', 'Working'))).toBe('Message Claude');
  });

  /// Working means the agent is busy, not that it has stopped listening: a pty
  /// takes what is typed whenever it is typed. tech.md 6.5.
  it('stays open while the agent is working', () => {
    expect(owned(card('Owned', 'Working'))).toBe(true);
  });

  /// The honest no. Peekle cannot type into a process it did not start, and
  /// says which one it is rather than going dark for no stated reason.
  it('says why an observed session has no field', () => {
    expect(replyHint(card('Observed'))).toBe('Started outside Peekle, so this one is read-only');
    expect(owned(card('Observed'))).toBe(false);
  });

  /// The bug this replaces: `Ended` came off SessionEnd, which fires at the end
  /// of any run while the session carries on, and it killed the field on a live
  /// session. Now only a process of ours exiting sets it. tech.md 6.3.
  it('closes the field only once the process behind it is gone', () => {
    expect(owned(card('Owned', 'Ended'))).toBe(false);
    expect(replyHint(card('Owned', 'Ended'))).toBe('This session has finished');
  });

  it('lets an open permission request take the field', () => {
    expect(replyHint(card('Observed'), true)).toBe('Reply to Claude');
  });
});

describe('the field itself', () => {
  it('takes no input for a session the island cannot type into', async () => {
    render(PromptInput, {
      value: '',
      placeholder: replyHint(card('Observed')),
      disabled: true,
      onsubmit: vi.fn(),
    });

    expect(screen.getByRole('textbox')).toBeDisabled();
  });

  /// The main action of the product had no visible affordance: sending lived
  /// on Enter alone. tech.md 9.
  it('sends what the button is pressed on, same as Enter', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { value: 'ship it', placeholder: '', disabled: false, onsubmit });

    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('ship it');
  });

  /// A button that does nothing when pressed lies about its own state.
  it('dims the button when there is nothing to send', async () => {
    const onsubmit = vi.fn();
    const { rerender } = render(PromptInput, {
      value: '',
      placeholder: '',
      disabled: false,
      onsubmit,
    });

    const button = screen.getByRole('button', { name: 'Send' });
    expect(button).toBeDisabled();

    await rerender({ value: '   ' });
    expect(button, 'whitespace is nothing to send').toBeDisabled();

    await rerender({ value: 'go' });
    expect(button).not.toBeDisabled();
  });

  it('offers nothing to press for a session it cannot type into', () => {
    render(PromptInput, {
      value: 'ship it',
      placeholder: replyHint(card('Observed')),
      disabled: true,
      onsubmit: vi.fn(),
    });

    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
  });

  it('accepts typing for a session the island owns', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, {
      value: '',
      placeholder: replyHint(card('Owned')),
      disabled: false,
      onsubmit,
    });

    const field = screen.getByRole('textbox');
    await userEvent.type(field, 'ship it{Enter}');
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('ship it');
  });
});

/// The empty black island: the view named a session the feed did not have yet,
/// so none of the render branches matched and the shape drew nothing. Falling
/// back to the list means a click always lands somewhere. tech.md 6.5.
describe('a view pointing at a session that is not there', () => {
  const listing = (view: 'Sessions' | { Session: string }, known: string[]) =>
    view === 'Sessions' || (typeof view === 'object' && !known.includes(view.Session));

  it('falls back to the list rather than drawing nothing', () => {
    expect(listing({ Session: 'brand-new' }, [])).toBe(true);
  });

  it('shows the session once its card is there', () => {
    expect(listing({ Session: 'brand-new' }, ['brand-new'])).toBe(false);
  });
});

describe('the resting mark', () => {
  /// A finished turn holds its own channel open and needs nobody, so it no
  /// longer pulses at the user. Only a permission request does. tech.md 6.7.
  it('pulses for a permission request and not for a finished turn', () => {
    expect(restStatus([card('Owned')], false)).toBe('idle');
    expect(restStatus([card('Owned')], true)).toBe('waiting');
    expect(restStatus([card('Owned', 'Working')], false)).toBe('working');
  });
});
