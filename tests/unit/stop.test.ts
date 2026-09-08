/**
 * S3 acceptance. The Stop path is the first place a keystroke reaches the
 * agent, so these check what leaves the island rather than how it looks.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { commands } from '$lib/bridge';
import { createIsland } from '$lib/features/island/island.svelte';
import Button from '$lib/ui/Button.svelte';
import PromptInput from '$lib/ui/PromptInput.svelte';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: { ...real.commands, sendMessage: vi.fn(), answerPrompt: vi.fn() },
  };
});

beforeEach(() => {
  vi.mocked(commands.sendMessage).mockClear();
  vi.mocked(commands.answerPrompt).mockClear();
});

/**
 * The one button under the field. An arrow while there is something to send,
 * a square while a turn runs, and never two controls to choose between.
 * tech.md 6.5 and 9.
 */
describe('the button under the field', () => {
  it('sends what was typed while nothing is running', async () => {
    const onsubmit = vi.fn();
    const onstop = vi.fn();
    render(PromptInput, { props: { value: 'ship it', onsubmit, onstop } });

    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('ship it');
    expect(onstop).not.toHaveBeenCalled();
  });

  it('becomes the way to end a turn while one is running', async () => {
    const onsubmit = vi.fn();
    const onstop = vi.fn();
    render(PromptInput, { props: { value: '', working: true, onsubmit, onstop } });

    const button = screen.getByRole('button', { name: 'Stop' });
    // Empty field or not: there is a turn to end, so the press does something.
    expect(button).not.toBeDisabled();
    await userEvent.click(button);
    expect(onstop).toHaveBeenCalledOnce();
    expect(onsubmit).not.toHaveBeenCalled();
  });

  /// Typing does not stop while the agent works -- the terminal queues the
  /// next message, and so does this. Only the button changes. tech.md 6.5.
  it('still sends on Enter while a turn is running', async () => {
    const onsubmit = vi.fn();
    const onstop = vi.fn();
    render(PromptInput, { props: { value: 'next up', working: true, onsubmit, onstop } });

    await userEvent.type(screen.getByRole('textbox'), '{Enter}');
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith('next up');
    expect(onstop).not.toHaveBeenCalled();
  });
});

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

describe('writing while the agent is busy', () => {
  /// The island hands the text to Rust, which picks the channel. It never
  /// decides the route itself: only Rust can see whether a pane exists at this
  /// instant. tech.md 6.5.
  it('sends the text for the session instead of answering nothing', () => {
    const island = createIsland('');
    island.answer('keep going', 's1');

    expect(commands.sendMessage).toHaveBeenCalledExactlyOnceWith('s1', 'keep going', []);
    expect(commands.answerPrompt).not.toHaveBeenCalled();
  });

  it('sends nothing at all when it does not know which session', () => {
    const island = createIsland('');
    island.answer('keep going');

    expect(commands.sendMessage).not.toHaveBeenCalled();
  });

  it('never sends whitespace', () => {
    const island = createIsland('');
    island.answer('   \n  ', 's1');

    expect(commands.sendMessage).not.toHaveBeenCalled();
  });
});
