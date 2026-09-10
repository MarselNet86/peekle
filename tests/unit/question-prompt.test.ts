/**
 * S16 acceptance. AskUserQuestion is answered one question at a time: two
 * `OptionList`s mounted together would both answer the same digit, so only
 * one is ever on screen. tech.md 6.14.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import QuestionPrompt from '$lib/ui/QuestionPrompt.svelte';
import type { Question } from '$lib/types/generated/Question';

const single: Question = {
  header: 'Framework',
  question: 'Which framework?',
  options: [
    { label: 'React', description: null },
    { label: 'Vue', description: null },
  ],
  multi_select: false,
};

const multi: Question = {
  header: 'Checks',
  question: 'Which checks should block the merge?',
  options: [
    { label: 'Lint', description: null },
    { label: 'Tests', description: null },
  ],
  multi_select: true,
};

describe('a single question', () => {
  it('answers on a click, with no Submit button needed', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onsubmit } });

    expect(screen.queryByRole('button', { name: 'Submit' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByText('React'));

    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      { question: 'Which framework?', labels: ['React'] },
    ]);
  });

  it('answers on its digit the same way', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onsubmit } });

    await userEvent.keyboard('2');
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      { question: 'Which framework?', labels: ['Vue'] },
    ]);
  });
});

describe('a multiSelect question', () => {
  it('needs an explicit Submit, and it stays disabled until something is checked', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [multi], onsubmit } });

    const submit = screen.getByRole('button', { name: 'Submit' });
    expect(submit).toBeDisabled();

    await userEvent.click(screen.getByText('Lint'));
    expect(submit).not.toBeDisabled();
    expect(onsubmit).not.toHaveBeenCalled();

    await userEvent.click(submit);
    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      { question: 'Which checks should block the merge?', labels: ['Lint'] },
    ]);
  });

  it('reports every box checked, not just the first', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [multi], onsubmit } });

    await userEvent.click(screen.getByText('Lint'));
    await userEvent.click(screen.getByText('Tests'));
    await userEvent.click(screen.getByRole('button', { name: 'Submit' }));

    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      { question: 'Which checks should block the merge?', labels: ['Lint', 'Tests'] },
    ]);
  });
});

describe('more than one question', () => {
  it('answers them in order and reports both only at the end', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single, multi], onsubmit } });

    expect(screen.getByText('1 / 2')).toBeInTheDocument();
    await userEvent.click(screen.getByText('React'));

    // The single-select answer moved things along on its own.
    expect(onsubmit).not.toHaveBeenCalled();
    expect(screen.getByText('2 / 2')).toBeInTheDocument();
    expect(screen.getByText('Which checks should block the merge?')).toBeInTheDocument();

    await userEvent.click(screen.getByText('Tests'));
    await userEvent.click(screen.getByRole('button', { name: 'Submit' }));

    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      { question: 'Which framework?', labels: ['React'] },
      { question: 'Which checks should block the merge?', labels: ['Tests'] },
    ]);
  });

  it('shows no progress line for a single question', () => {
    render(QuestionPrompt, { props: { questions: [single] } });
    expect(screen.queryByText('1 / 1')).not.toBeInTheDocument();
  });
});

/**
 * The answer that is not on the list. AskUserQuestion always lets a person
 * write their own, and without it the only way out of four answers that do not
 * fit is to pick the nearest one. tech.md 6.14.
 */
describe('an answer of your own', () => {
  it('is offered on every question, last', () => {
    render(QuestionPrompt, { props: { questions: [single] } });

    const rows = screen.getAllByText(/^(React|Vue|Other)$/);
    expect(rows.map((row) => row.textContent)).toEqual(['React', 'Vue', 'Other']);
  });

  /// Picking the row is not an answer: the answer is what gets written in the
  /// field it opens. Submitting on the pick would send the word "Other".
  it('opens a field and answers nothing by itself', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onsubmit } });

    expect(screen.queryByPlaceholderText('Type your answer…')).not.toBeInTheDocument();
    await userEvent.click(screen.getByText('Other'));

    expect(screen.getByPlaceholderText('Type your answer…')).toBeInTheDocument();
    expect(onsubmit).not.toHaveBeenCalled();
  });

  it('sends what was written as the answer', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onsubmit } });

    await userEvent.click(screen.getByText('Other'));
    await userEvent.type(screen.getByPlaceholderText('Type your answer…'), 'Svelte 5');
    await userEvent.keyboard('{Enter}');

    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      { question: 'Which framework?', labels: ['Svelte 5'] },
    ]);
  });

  /// The list listens on the window, so a digit typed into the field used to
  /// pick a row instead of landing in the answer. tech.md 9.
  it('takes digits as text once the field is open', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onsubmit } });

    await userEvent.click(screen.getByText('Other'));
    await userEvent.type(screen.getByPlaceholderText('Type your answer…'), 'Svelte 5');

    expect(onsubmit).not.toHaveBeenCalled();
    expect(screen.getByPlaceholderText('Type your answer…')).toHaveValue('Svelte 5');
  });

  it('sends nothing while the field is empty', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onsubmit } });

    await userEvent.click(screen.getByText('Other'));
    await userEvent.keyboard('{Enter}');
    expect(onsubmit).not.toHaveBeenCalled();

    await userEvent.type(screen.getByPlaceholderText('Type your answer…'), '   ');
    await userEvent.keyboard('{Enter}');
    expect(onsubmit).not.toHaveBeenCalled();
  });

  /// In a multiSelect question it is one more answer, not a replacement for
  /// the boxes that are checked.
  it('stands beside the boxes that are checked', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [multi], onsubmit } });

    await userEvent.click(screen.getByText('Lint'));
    await userEvent.click(screen.getByText('Other'));
    await userEvent.type(screen.getByPlaceholderText('Type your answer…'), 'Typecheck');
    await userEvent.click(screen.getByRole('button', { name: 'Submit' }));

    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      {
        question: 'Which checks should block the merge?',
        labels: ['Lint', 'Typecheck'],
      },
    ]);
  });

  /// Enter is the whole gesture, and it means the same thing next to a
  /// Submit button as it does without one: send this answer.
  it('sends on Enter even where a Submit button stands beside it', async () => {
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [multi], onsubmit } });

    await userEvent.click(screen.getByText('Lint'));
    await userEvent.click(screen.getByText('Other'));
    await userEvent.type(screen.getByPlaceholderText('Type your answer…'), 'Typecheck');
    await userEvent.keyboard('{Enter}');

    expect(onsubmit).toHaveBeenCalledExactlyOnceWith([
      {
        question: 'Which checks should block the merge?',
        labels: ['Lint', 'Typecheck'],
      },
    ]);
  });

  /// Checking it and writing nothing is not an answer, however many boxes are
  /// beside it: the row on its own says nothing.
  it('is not an answer while it is empty, even when checked', async () => {
    render(QuestionPrompt, { props: { questions: [multi] } });

    await userEvent.click(screen.getByText('Other'));
    expect(screen.getByRole('button', { name: 'Submit' })).toBeDisabled();
  });
});

/**
 * A way out of the question itself. Without it the only way past a question
 * with nothing right in it is to answer it wrongly. tech.md 6.14.
 */
describe('closing a question instead of answering it', () => {
  it('offers a cross, and it answers nothing', async () => {
    const onclose = vi.fn();
    const onsubmit = vi.fn();
    render(QuestionPrompt, { props: { questions: [single], onclose, onsubmit } });

    await userEvent.click(screen.getByRole('button', { name: 'Close the question' }));

    expect(onclose).toHaveBeenCalledOnce();
    expect(onsubmit).not.toHaveBeenCalled();
  });

  /// Nothing to close it into: a caller that does not take a refusal is not
  /// given a control that would do nothing.
  it('draws no cross where there is nowhere to close to', () => {
    render(QuestionPrompt, { props: { questions: [single] } });

    expect(screen.queryByRole('button', { name: 'Close the question' })).not.toBeInTheDocument();
  });
});
