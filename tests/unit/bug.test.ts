/**
 * v87.4 acceptance: the bug button asks first. tech.md 6.22.
 *
 * Criteria, from the owner's request: pressing the bug button folds the
 * island into a question, "Found a bug? Tell us.", with two buttons, Write
 * and Cancel. Write opens Telegram and the island closes completely.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/index.svelte';
import { shapeBounds } from '$lib/logic/shape';
import BugPanel from '$lib/ui/BugPanel.svelte';

afterEach(() => {
  i18n.set('en');
});

describe('the bug panel', () => {
  it('asks whether there is a bug to tell, with Write white and Cancel dark', () => {
    render(BugPanel);
    expect(screen.getByText('Found a bug?')).toBeInTheDocument();
    expect(screen.getByText('Tell us about it.')).toBeInTheDocument();

    const write = screen.getByRole('button', { name: 'Write' });
    const cancel = screen.getByRole('button', { name: 'Cancel' });
    expect(write).toHaveAttribute('data-variant', 'prominent');
    expect(cancel).toHaveAttribute('data-variant', 'muted');
    expect(write.compareDocumentPosition(cancel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('asks in Russian when the island speaks it', () => {
    i18n.set('ru');
    render(BugPanel);
    expect(screen.getByText('Нашли баг?')).toBeInTheDocument();
    expect(screen.getByText('Сообщите нам.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Написать' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Отмена' })).toBeInTheDocument();
  });

  it('answers write and cancel, and Escape is cancel', async () => {
    const onwrite = vi.fn();
    const oncancel = vi.fn();
    render(BugPanel, { props: { onwrite, oncancel } });

    await userEvent.click(screen.getByRole('button', { name: 'Write' }));
    expect(onwrite).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(oncancel).toHaveBeenCalledTimes(1);
    await userEvent.keyboard('{Escape}');
    expect(oncancel).toHaveBeenCalledTimes(2);
  });

  it('takes no second answer while Telegram opens', async () => {
    const oncancel = vi.fn();
    render(BugPanel, { props: { busy: true, oncancel } });
    expect(screen.getByRole('button', { name: /Write/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();
    await userEvent.keyboard('{Escape}');
    expect(oncancel).not.toHaveBeenCalled();
  });

  it('says why Telegram did not open, in the line under the title', () => {
    render(BugPanel, { props: { error: 'Could not open Telegram' } });
    expect(screen.getByText('Could not open Telegram')).toBeInTheDocument();
    expect(screen.queryByText('Tell us about it.')).not.toBeInTheDocument();
  });

  it('opens the island just enough, the size of the quit question', () => {
    const notch = { width: 185, height: 34 };
    expect(shapeBounds('Bug', notch)).toEqual(shapeBounds('Quit', notch));
  });
});
