/**
 * S23 acceptance. The gear opens one setting, the switch says what is so and
 * nothing more, and neither of them moves before the system answers.
 * tech.md 6.17.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { NOTIFY_HINT } from '$lib/features/notify/notify.svelte';
import IconButton from '$lib/ui/IconButton.svelte';
import Toggle from '$lib/ui/Toggle.svelte';

describe('the gear', () => {
  it('says what it does for a reader who cannot see a gear', () => {
    render(IconButton, { props: { name: 'gear', title: 'Settings' } });
    expect(screen.getByRole('button', { name: 'Settings' })).toBeTruthy();
  });

  /// Lit while what it opened stands open: a control that opens something and
  /// then looks untouched reads as one that did nothing.
  it('shows whether what it opened is open', () => {
    const { rerender } = render(IconButton, {
      props: { name: 'gear', title: 'Settings', pressed: false },
    });
    expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('false');

    rerender({ name: 'gear', title: 'Settings', pressed: true });
    expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('true');
  });

  it('calls back on a press', async () => {
    const onclick = vi.fn();
    render(IconButton, { props: { name: 'gear', title: 'Settings', onclick } });

    await userEvent.click(screen.getByRole('button'));
    expect(onclick).toHaveBeenCalledTimes(1);
  });
});

describe('the switch', () => {
  it('asks for the opposite of where it stands, once', async () => {
    const onchange = vi.fn();
    render(Toggle, {
      props: { label: 'Notify when a turn ends', checked: false, onchange },
    });

    await userEvent.click(screen.getByRole('switch'));
    expect(onchange).toHaveBeenCalledTimes(1);
    expect(onchange).toHaveBeenCalledWith(true);
  });

  it('stands on what is so, not on what was asked for', () => {
    render(Toggle, { props: { label: 'Notify when a turn ends', checked: true } });
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('true');
  });

  /// A switch that moves before the answer lands is a switch that lies about
  /// the state, so it takes no presses while it waits. tech.md 9.
  it('takes no presses while the system is still answering', async () => {
    const onchange = vi.fn();
    render(Toggle, {
      props: { label: 'Notify when a turn ends', checked: false, busy: true, onchange },
    });

    await userEvent.click(screen.getByRole('switch'));
    expect(onchange).not.toHaveBeenCalled();
  });

  it('carries the line about where this is really decided', () => {
    render(Toggle, {
      props: { label: 'Notify when a turn ends', checked: true, hint: NOTIFY_HINT },
    });
    expect(screen.getByText(NOTIFY_HINT)).toBeTruthy();
  });
});

/// macOS swallows forbidden banners silently and never tells the app, so the
/// line under the switch names where that is checked rather than promising
/// that everything works. tech.md 6.17.
describe('the copy under the switch', () => {
  it('sends the reader to the system rather than claiming success', () => {
    expect(NOTIFY_HINT).toContain('System Settings');
    expect(NOTIFY_HINT).not.toMatch(/enabled|working|on now/i);
  });
});
