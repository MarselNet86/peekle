/**
 * S16 acceptance, the plumbing between QuestionPrompt and the hook answer.
 * tech.md 6.14.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { commands, events } from '$lib/bridge';
import { createIsland } from '$lib/features/island/island.svelte';
import { isPermission, isQuestion } from '$lib/features/permission/permission.svelte';
import type { PromptRequest } from '$lib/types/generated/PromptRequest';

vi.mock('$lib/bridge', async (original) => {
  const real = await original<typeof import('$lib/bridge')>();
  return {
    ...real,
    commands: { ...real.commands, answerPrompt: vi.fn(), getState: vi.fn() },
    events: { ...real.events, onPromptOpen: vi.fn() },
  };
});

/** Hands back the handler Rust would call on `peekle://prompt-open`. */
function openHandler(): (request: PromptRequest) => void {
  const [handler] = vi.mocked(events.onPromptOpen).mock.calls.at(-1) ?? [];
  if (!handler) throw new Error('nothing subscribed to prompt-open');
  return handler;
}

beforeEach(() => {
  vi.mocked(commands.answerPrompt).mockClear();
  vi.mocked(commands.getState).mockClear().mockResolvedValue(null);
  vi.mocked(events.onPromptOpen)
    .mockClear()
    .mockResolvedValue(() => {});
});

const question: PromptRequest = {
  id: '01J0',
  kind: 'Question',
  session: { session_id: 's', cwd: '/x/peekle', project: 'peekle', pid: null, tty: null },
  title: 'Which framework?',
  tool: 'AskUserQuestion',
  last_message: null,
  detail: null,
  options: [],
  questions: [
    {
      header: 'Framework',
      question: 'Which framework?',
      options: [{ label: 'React', description: null }],
      multi_select: false,
    },
  ],
  allow_free_text: false,
  created_at: 0,
  expires_at: 0,
};

describe('telling a question apart from a permission', () => {
  it('is a question and not a permission', () => {
    expect(isQuestion(question)).toBe(true);
    expect(isPermission(question)).toBe(false);
  });

  it('is neither for anything else, including null', () => {
    expect(isQuestion({ ...question, kind: 'Permission' })).toBe(false);
    expect(isQuestion(null)).toBe(false);
  });
});

describe('answering every question at once', () => {
  it('sends the answers and carries no choice or text', async () => {
    const island = createIsland('');
    await island.start();
    openHandler()(question);

    const answers = [{ question: 'Which framework?', labels: ['React'] }];
    island.answerQuestions(answers);

    expect(commands.answerPrompt).toHaveBeenCalledExactlyOnceWith({
      prompt_id: '01J0',
      choice: null,
      text: null,
      answers,
    });
    // Answered means settled: the field this prompt held clears immediately,
    // the same way an answered permission does. tech.md 6.5.
    expect(island.prompt).toBeNull();
  });

  it('is a no-op with no prompt open', async () => {
    const island = createIsland('');
    await island.start();

    island.answerQuestions([{ question: 'Which framework?', labels: ['React'] }]);
    expect(commands.answerPrompt).not.toHaveBeenCalled();
  });
});
