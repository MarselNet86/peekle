/**
 * S3 acceptance. The ladder decides where typed text goes, and the field has
 * to tell the truth about which rung it is on. tech.md 6.5.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import PromptInput from '$lib/ui/PromptInput.svelte';
import { restStatus } from '$lib/logic/rest';
import type { Delivery } from '$lib/types/generated/Delivery';
import type { SessionCard } from '$lib/types/generated/SessionCard';

/// The same expression the island renders its placeholder from. Kept here so
/// the wording is pinned by a test rather than by whoever edits the markup.
function replyHint(delivery: Delivery | null, prompt = false): string {
  if (prompt) return 'Reply to Claude';
  if (delivery === null) return 'Send to Claude';
  if (delivery === 'Unreachable') return 'This session has ended';
  if (delivery === 'TurnBoundary') return 'Type now, it goes when Claude stops';
  return 'Message Claude';
}

const pane: Delivery = { Tmux: { session: 'work', window: 0, pane: 1 } };

describe('what the field promises', () => {
  /// The two channels differ in when the text lands, and hiding that would be
  /// a promise the product cannot keep. tech.md 6.5.
  it('names the channel it actually has', () => {
    expect(replyHint(pane)).toBe('Message Claude');
    expect(replyHint('TurnBoundary')).toBe('Type now, it goes when Claude stops');
  });

  /// The competitor goes dark outside tmux. A session in an IDE extension is
  /// slower, not mute, so the field stays live. tech.md 17.1.
  it('stays live for a session with no pane', () => {
    expect(replyHint('TurnBoundary')).not.toBe(replyHint('Unreachable'));
  });

  it('says so only when there is genuinely nowhere to deliver', () => {
    expect(replyHint('Unreachable')).toBe('This session has ended');
  });

  it('lets an open permission request take the field', () => {
    expect(replyHint(pane, true)).toBe('Reply to Claude');
  });
});

describe('the field itself', () => {
  it('takes no input when nothing can be delivered', async () => {
    render(PromptInput, {
      value: '',
      placeholder: replyHint('Unreachable'),
      disabled: true,
      onsubmit: vi.fn(),
    });

    const field = screen.getByRole('textbox');
    expect(field).toBeDisabled();
  });

  /// Every rung except Unreachable accepts typing, including the slow one.
  it('accepts typing on either channel', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, {
      value: '',
      placeholder: replyHint('TurnBoundary'),
      disabled: false,
      onsubmit,
    });

    const field = screen.getByRole('textbox');
    await userEvent.type(field, 'ship it{Enter}');
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('ship it');
  });
});

describe('the resting mark', () => {
  const card = (status: SessionCard['status']): SessionCard => ({
    session: { session_id: 's', cwd: '/x/p', project: 'p', pid: null, tty: null },
    title: 'x',
    status,
    entries: [],
    updated_at: 0,
  });

  /// A finished turn holds its own channel open and needs nobody, so it no
  /// longer pulses at the user. Only a permission request does. tech.md 6.7.
  it('pulses for a permission request and not for a finished turn', () => {
    expect(restStatus([card('Idle')], false)).toBe('idle');
    expect(restStatus([card('Idle')], true)).toBe('waiting');
    expect(restStatus([card('Working')], false)).toBe('working');
  });
});
