/**
 * S5 acceptance. Two parallel sessions have to be visible at once and told
 * apart by status, and navigation has to go both ways.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { sessionOf } from '$lib/features/sessions/sessions.svelte';
import SessionRow from '$lib/ui/SessionRow.svelte';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { SessionCard } from '$lib/types/generated/SessionCard';
import type { SessionStatus } from '$lib/types/generated/SessionStatus';

const card = (status: SessionStatus, over: Partial<SessionCard> = {}): SessionCard => ({
  session: { session_id: 's1', cwd: '/Users/x/peekle', project: 'peekle' },
  title: 'Refactor the panel code',
  status,
  entries: [],
  updated_at: 0,
  ...over,
});

describe('the view a session id comes from', () => {
  it('reads the id out of a session view and nothing else', () => {
    expect(sessionOf({ Session: 'abc' } as IslandView)).toBe('abc');
    expect(sessionOf('Sessions')).toBeUndefined();
    expect(sessionOf('Collapsed')).toBeUndefined();
    expect(sessionOf('Pill')).toBeUndefined();
  });
});

describe('SessionRow', () => {
  it('shows the first user turn as the title', () => {
    render(SessionRow, { props: { card: card('Working') } });
    expect(screen.getByText('Refactor the panel code')).toBeInTheDocument();
  });

  it('falls back to the project when no turn has been seen yet', () => {
    render(SessionRow, { props: { card: card('Working', { title: '' }) } });
    expect(screen.getAllByText(/peekle/).length).toBeGreaterThan(0);
  });

  it('tells every status apart', () => {
    for (const [status, words] of [
      ['Working', 'working'],
      ['WaitingOnUser', 'waiting on you'],
      ['Idle', 'idle'],
      ['Ended', 'ended'],
    ] as [SessionStatus, string][]) {
      const { container, unmount } = render(SessionRow, { props: { card: card(status) } });

      expect(container.querySelector('.row')?.getAttribute('data-status')).toBe(status);
      expect(screen.getByText(new RegExp(words))).toBeInTheDocument();
      unmount();
    }
  });

  it('opens the session on a click', async () => {
    const onopen = vi.fn();
    render(SessionRow, { props: { card: card('Working'), onopen } });

    await userEvent.click(screen.getByRole('button'));
    expect(onopen).toHaveBeenCalledOnce();
  });

  it('is a button, so the keyboard reaches it like the mouse does', async () => {
    const onopen = vi.fn();
    render(SessionRow, { props: { card: card('Working'), onopen } });

    const row = screen.getByRole('button');
    row.focus();
    await userEvent.keyboard('{Enter}');
    expect(onopen).toHaveBeenCalledOnce();
  });
});
