/**
 * The way to the developer. tech.md 6.22.
 *
 * Acceptance, from the change: a button beside the gear, a short line raised
 * on hover that says what pressing it does, and a press that opens the
 * developer's Telegram. The line is the primitive's own, so what it says and
 * when it stands are read here; where it stands, and that it moves no row, is
 * layout and is read in the e2e.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import IconButton from '$lib/ui/IconButton.svelte';

const HINT = 'Tell the developer what broke. Opens Telegram.';

const bug = (props: Record<string, unknown> = {}) =>
  render(IconButton, { props: { name: 'bug', title: 'Report a bug', hint: HINT, ...props } });

describe('the bug button', () => {
  it('says what it does for a reader who sees no bug', () => {
    bug();
    expect(screen.getByRole('button', { name: 'Report a bug' })).toBeTruthy();
  });

  /// The pointer is what asks the question, so the answer stands while the
  /// pointer is there and goes when it leaves. Nothing is on screen before
  /// it is asked for: a line that stands permanently is a caption, and the
  /// island has no room for captions.
  it('raises its line on hover and takes it back on leaving', async () => {
    bug();
    expect(screen.queryByRole('tooltip')).toBeNull();

    const button = screen.getByRole('button', { name: 'Report a bug' });
    await userEvent.hover(button);
    expect(screen.getByRole('tooltip').textContent).toBe(HINT);

    await userEvent.unhover(button);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  /// Whoever reaches the button by keyboard gets the same answer: a hint that
  /// only a pointer can raise is a hint half the ways in never see.
  it('raises it for the keyboard as well', async () => {
    bug();

    await userEvent.tab();
    expect(screen.getByRole('button', { name: 'Report a bug' })).toBe(document.activeElement);
    expect(screen.getByRole('tooltip').textContent).toBe(HINT);
  });

  /// And the line is named as the button's own: read aloud, the button says
  /// what it is and then what it does.
  it('hangs the line off the button that raised it', async () => {
    bug();
    const button = screen.getByRole('button', { name: 'Report a bug' });
    await userEvent.hover(button);

    const hint = screen.getByRole('tooltip');
    expect(button.getAttribute('aria-describedby')).toBe(hint.id);
    expect(hint.id).toBeTruthy();
  });

  /// The gear next to it was given nothing to say, and says nothing: the hint
  /// is one button's, not every button's.
  it('leaves a button with nothing to say silent', async () => {
    render(IconButton, { props: { name: 'settings', title: 'Settings' } });

    await userEvent.hover(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('opens the chat on a press', async () => {
    const onclick = vi.fn();
    bug({ onclick });

    await userEvent.click(screen.getByRole('button', { name: 'Report a bug' }));
    expect(onclick).toHaveBeenCalledTimes(1);
  });
});
