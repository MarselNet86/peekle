/**
 * S3 acceptance. The Stop path is the first place a keystroke reaches the
 * agent, so these check what leaves the island rather than how it looks.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import Button from '$lib/ui/Button.svelte';
import PromptInput from '$lib/ui/PromptInput.svelte';

describe('the reply field', () => {
  it('sends the typed text on Enter', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { props: { value: 'open a PR', onsubmit } });

    await userEvent.type(screen.getByRole('textbox'), '{Enter}');
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('open a PR');
  });

  it('breaks the line on Shift+Enter and sends nothing', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { props: { onsubmit } });

    const field = screen.getByRole('textbox') as HTMLTextAreaElement;
    await userEvent.type(field, 'one{Shift>}{Enter}{/Shift}two');

    expect(onsubmit).not.toHaveBeenCalled();
    expect(field.value).toBe('one\ntwo');
  });

  it('cancels on Escape, which ends the turn normally', async () => {
    const onescape = vi.fn();
    const onsubmit = vi.fn();
    render(PromptInput, { props: { value: 'half a thought', onsubmit, onescape } });

    await userEvent.type(screen.getByRole('textbox'), '{Escape}');
    expect(onescape).toHaveBeenCalledOnce();
    expect(onsubmit).not.toHaveBeenCalled();
  });

  it('carries a reply that is not ASCII through untouched', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { props: { value: 'продолжай рефакторинг', onsubmit } });

    await userEvent.type(screen.getByRole('textbox'), '{Enter}');
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('продолжай рефакторинг');
  });

  it('goes quiet when the session is not waiting on anybody', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { props: { value: 'anyone there', disabled: true, onsubmit } });

    const field = screen.getByRole('textbox');
    expect(field).toBeDisabled();
    await userEvent.type(field, '{Enter}');
    expect(onsubmit).not.toHaveBeenCalled();
  });
});

describe('the choice buttons', () => {
  it('reports the choice once per click', async () => {
    const onclick = vi.fn();
    render(Button, { props: { label: 'Continue', variant: 'primary', onclick } });

    await userEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(onclick).toHaveBeenCalledOnce();
  });

  it('stays silent while disabled', async () => {
    const onclick = vi.fn();
    render(Button, { props: { label: 'Finish', disabled: true, onclick } });

    await userEvent.click(screen.getByRole('button', { name: 'Finish' }));
    expect(onclick).not.toHaveBeenCalled();
  });

  /// The panel must not become key because a button exists. tech.md 6.7.
  it('does not put itself in the way of the keyboard', () => {
    const { container } = render(Button, { props: { label: 'Finish' } });
    const button = container.querySelector('button');

    expect(button?.hasAttribute('autofocus')).toBe(false);
    expect(document.activeElement).not.toBe(button);
  });
});
