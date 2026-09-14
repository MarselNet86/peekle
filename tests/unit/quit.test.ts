/**
 * v85 acceptance: quitting asks first. tech.md 6.29.
 *
 * Criteria, from the owner's request: ⌥⌘Q and a button after the gear both
 * open the island just enough to ask whether to quit, with two buttons, Yes
 * dark and No white.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/index.svelte';
import { shapeBounds } from '$lib/logic/shape';
import IconButton from '$lib/ui/IconButton.svelte';
import QuitPanel from '$lib/ui/QuitPanel.svelte';

afterEach(() => {
  i18n.set('en');
});

describe('the quit panel', () => {
  it('asks whether to quit, with Yes dark and No white', () => {
    render(QuitPanel);
    expect(screen.getByText('Quit Peekle?')).toBeInTheDocument();
    expect(screen.getByText('Are you sure you want to quit?')).toBeInTheDocument();

    const yes = screen.getByRole('button', { name: 'Yes' });
    const no = screen.getByRole('button', { name: 'No' });
    expect(yes).toHaveAttribute('data-variant', 'muted');
    expect(no).toHaveAttribute('data-variant', 'prominent');
    expect(yes.compareDocumentPosition(no) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('asks in Russian when the island speaks it', () => {
    i18n.set('ru');
    render(QuitPanel);
    expect(screen.getByText('Вы уверены, что хотите выйти?')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Да' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Нет' })).toBeInTheDocument();
  });

  it('answers yes and no, and Escape is no', async () => {
    const onyes = vi.fn();
    const onno = vi.fn();
    render(QuitPanel, { props: { onyes, onno } });

    await userEvent.click(screen.getByRole('button', { name: 'Yes' }));
    expect(onyes).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole('button', { name: 'No' }));
    expect(onno).toHaveBeenCalledTimes(1);
    await userEvent.keyboard('{Escape}');
    expect(onno).toHaveBeenCalledTimes(2);
  });

  it('takes no second answer while it folds away', async () => {
    const onyes = vi.fn();
    const onno = vi.fn();
    render(QuitPanel, { props: { busy: true, onyes, onno } });
    expect(screen.getByRole('button', { name: 'Yes' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'No' })).toBeDisabled();
    await userEvent.keyboard('{Escape}');
    expect(onno).not.toHaveBeenCalled();
  });

  it('opens the island just enough, the size of the permission panel', () => {
    const notch = { width: 185, height: 34 };
    expect(shapeBounds('Quit', notch)).toEqual(shapeBounds('Ask', notch));
    expect(shapeBounds('Quit', notch).height).toBeLessThan(shapeBounds('Sessions', notch).height);
  });
});

describe('the way out beside the gear', () => {
  it('says what it does', () => {
    render(IconButton, { props: { name: 'quit', title: 'Quit Peekle' } });
    expect(screen.getByRole('button', { name: 'Quit Peekle' })).toBeInTheDocument();
  });
});
