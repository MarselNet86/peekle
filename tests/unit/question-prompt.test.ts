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
