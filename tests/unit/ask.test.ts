/**
 * v59 acceptance. A permission is asked by a compact panel, not by the whole
 * dialogue: two lines, two buttons, twenty seconds, and one press to go and
 * read what is being agreed to. tech.md 6.7.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ASK_SECS, secsLeft } from '$lib/features/permission/permission.svelte';
import { WINDOW, shapeBounds } from '$lib/logic/shape';
import AskPanel from '$lib/ui/AskPanel.svelte';
import type { IslandView } from '$lib/types/generated/IslandView';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';

const request: PromptRequest = {
  id: '01J0',
  kind: 'Permission',
  session: {
    session_id: 's1',
    cwd: '/Users/mars/peekle',
    project: 'peekle',
    pid: 4242,
    tty: null,
  },
  title: 'Bash needs permission',
  last_message: null,
  detail: 'cargo test --workspace',
  options: [
    { id: 'allow_once', label: 'Allow once', hint: null, kind: 'AllowOnce' },
    { id: 'deny', label: 'Deny', hint: null, kind: 'Deny' },
  ],
  questions: [],
  allow_free_text: true,
  created_at: Date.now(),
  expires_at: Date.now() + 300_000,
};

describe('the Ask view', () => {
  it('opens the island a little, not onto the whole feed', () => {
    const notch = { width: 200, height: 32 };
    const ask = shapeBounds('Ask' as IslandView, notch);
    const session = shapeBounds({ Session: 's1' } as IslandView, notch);

    expect(ask.height).toBeLessThan(session.height);
    expect(ask.width).toBeLessThanOrEqual(WINDOW.width);
    expect(ask.height).toBeGreaterThan(notch.height);
  });
});

describe('secsLeft', () => {
  it('counts the panel down from twenty', () => {
    expect(secsLeft(1_000_000, 1_000_000)).toBe(ASK_SECS);
    expect(secsLeft(1_000_000, 1_005_000)).toBe(15);
  });

  it('never goes below zero or above the whole, whatever the clocks say', () => {
    expect(secsLeft(1_000_000, 1_999_000)).toBe(0);
    expect(secsLeft(1_000_000, 900_000)).toBe(ASK_SECS);
    expect(secsLeft(Number.NaN, 1_000)).toBe(ASK_SECS);
  });
});

describe('AskPanel', () => {
  it('says what wants permission and what it is about to run', () => {
    render(AskPanel, { props: { request } });

    expect(screen.getByText('Bash needs permission')).toBeInTheDocument();
    expect(screen.getByText('cargo test --workspace')).toBeInTheDocument();
  });

  it('answers with the two buttons', async () => {
    const allow = vi.fn();
    const deny = vi.fn();
    render(AskPanel, { props: { request, onallow: allow, ondeny: deny } });

    await userEvent.click(screen.getByRole('button', { name: 'Allow' }));
    await userEvent.click(screen.getByRole('button', { name: 'Deny' }));

    expect(allow).toHaveBeenCalledOnce();
    expect(deny).toHaveBeenCalledOnce();
  });

  it('answers from the keyboard, the same keys the row takes', async () => {
    const allow = vi.fn();
    const deny = vi.fn();
    render(AskPanel, { props: { request, onallow: allow, ondeny: deny } });

    await userEvent.keyboard('2');
    await userEvent.keyboard('1');

    expect(allow).toHaveBeenCalledOnce();
    expect(deny).toHaveBeenCalledOnce();
  });

  it('goes to the session when pressed anywhere but the buttons', async () => {
    const open = vi.fn();
    const allow = vi.fn();
    const { container } = render(AskPanel, {
      props: { request, onopen: open, onallow: allow },
    });

    const title = screen.getByText('Bash needs permission');
    await userEvent.click(title);
    expect(open).toHaveBeenCalledOnce();

    // The buttons answer and nothing else: a press on Allow must never also
    // carry the person off to the feed.
    await userEvent.click(screen.getByRole('button', { name: 'Allow' }));
    expect(allow).toHaveBeenCalledOnce();
    expect(open).toHaveBeenCalledOnce();
    expect(container.querySelector('.leak')).not.toBeNull();
  });

  it('shows the time it has left', () => {
    render(AskPanel, { props: { request: { ...request, created_at: Date.now() - 5_000 } } });

    expect(screen.getByText('15s')).toBeInTheDocument();
  });
});
