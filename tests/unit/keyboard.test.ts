/**
 * Component tests for the keyboard contract of section 9: Enter sends,
 * Shift+Enter breaks the line, Escape cancels, arrows and digits move the
 * selection.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import OptionList from '$lib/ui/OptionList.svelte';
import PromptInput from '$lib/ui/PromptInput.svelte';
import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';

const options: ChoiceOption[] = [
  { id: 'allow_once', label: 'Allow once', hint: null, kind: 'AllowOnce' },
  { id: 'allow_always', label: 'Allow for this session', hint: null, kind: 'AllowAlways' },
  { id: 'deny', label: 'Deny', hint: 'Type a reason first', kind: 'Deny' },
];

describe('PromptInput', () => {
  it('sends on Enter', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { props: { value: 'ship it', onsubmit } });

    await userEvent.type(screen.getByRole('textbox'), '{Enter}');
    expect(onsubmit).toHaveBeenCalledWith('ship it');
  });

  it('breaks the line on Shift+Enter and does not send', async () => {
    const onsubmit = vi.fn();
    render(PromptInput, { props: { onsubmit } });

    const field = screen.getByRole('textbox') as HTMLTextAreaElement;
    await userEvent.type(field, 'one{Shift>}{Enter}{/Shift}two');

    expect(onsubmit).not.toHaveBeenCalled();
    expect(field.value).toBe('one\ntwo');
  });

  it('cancels on Escape', async () => {
    const onescape = vi.fn();
    const onsubmit = vi.fn();
    render(PromptInput, { props: { onescape, onsubmit } });

    await userEvent.type(screen.getByRole('textbox'), '{Escape}');
    expect(onescape).toHaveBeenCalledOnce();
    expect(onsubmit).not.toHaveBeenCalled();
  });

  it('takes focus on mount, so the user can type without clicking', () => {
    render(PromptInput, { props: {} });
    expect(screen.getByRole('textbox')).toHaveFocus();
  });
});

describe('OptionList', () => {
  it('renders every option with its index', () => {
    render(OptionList, { props: { options } });

    for (const option of options) {
      expect(screen.getByText(option.label)).toBeInTheDocument();
    }
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('picks an option by its digit', async () => {
    const onselect = vi.fn();
    render(OptionList, { props: { options, onselect } });

    await userEvent.keyboard('2');
    expect(onselect).toHaveBeenCalledWith('allow_always');
  });

  it('ignores a digit with no option behind it', async () => {
    const onselect = vi.fn();
    render(OptionList, { props: { options, onselect } });

    await userEvent.keyboard('9');
    expect(onselect).not.toHaveBeenCalled();
  });

  it('moves the selection with the arrow keys', async () => {
    const onselect = vi.fn();
    render(OptionList, { props: { options, selected: 'allow_once', onselect } });

    const first = screen.getByText('Allow once').closest('button');
    first?.focus();
    await userEvent.keyboard('{ArrowDown}');

    expect(onselect).toHaveBeenCalledWith('allow_always');
  });

  /// A permission is one choice, an AskUserQuestion multiSelect question is
  /// several: `multiple` swaps radio-and-done for checkboxes that keep every
  /// pick, rather than one overwriting the last. tech.md 6.14.
  describe('multiple mode', () => {
    it('keeps every checked option rather than replacing the last one', async () => {
      render(OptionList, { props: { options, multiple: true } });

      await userEvent.click(screen.getByText('Allow once'));
      await userEvent.click(screen.getByText('Deny'));

      // Two independent picks, not one overwriting the other.
      expect(screen.getByText('Allow once').closest('[role="checkbox"]')).toHaveAttribute(
        'data-state',
        'checked',
      );
      expect(screen.getByText('Deny').closest('[role="checkbox"]')).toHaveAttribute(
        'data-state',
        'checked',
      );
    });

    it('toggles a row on its digit instead of locking in a single answer', async () => {
      render(OptionList, { props: { options, multiple: true } });

      await userEvent.keyboard('1');
      expect(screen.getByText('Allow once').closest('[role="checkbox"]')).toHaveAttribute(
        'data-state',
        'checked',
      );

      await userEvent.keyboard('1');
      expect(screen.getByText('Allow once').closest('[role="checkbox"]')).toHaveAttribute(
        'data-state',
        'unchecked',
      );
    });
  });
});
