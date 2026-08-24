<script lang="ts">
  /**
   * One to four AskUserQuestion questions, answered one at a time. tech.md
   * 6.14.
   *
   * One at a time and not all at once: `OptionList` binds a global digit
   * shortcut to whatever is on screen, and two mounted together would answer
   * the same keystroke twice.
   */
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';
  import type { Question } from '$lib/types/generated/Question';
  import type { QuestionAnswer } from '$lib/types/generated/QuestionAnswer';
  import Button from './Button.svelte';
  import OptionList from './OptionList.svelte';

  let {
    questions,
    onsubmit,
  }: {
    questions: Question[];
    onsubmit?: (answers: QuestionAnswer[]) => void;
  } = $props();

  /** AskUserQuestion identifies an option by its label, not by an id: the
   * label is what `answers` reports back. tech.md 6.14. */
  function toOptions(question: Question): ChoiceOption[] {
    return question.options.map((option) => ({
      id: option.label,
      label: option.label,
      hint: option.description,
      kind: 'Custom',
    }));
  }

  let index = $state(0);
  let selected = $state('');
  let values = $state<string[]>([]);
  let given: QuestionAnswer[] = $state([]);

  const question = $derived(questions[index]);
  const last = $derived(index === questions.length - 1);
  const answered = $derived(question.multi_select ? values.length > 0 : selected !== '');

  /** Records this question's answer and moves to the next, or submits every
   * answer once the last one is in. */
  function advance() {
    if (!answered) return;

    const next = [
      ...given,
      {
        question: question.question,
        labels: question.multi_select ? values : [selected],
      },
    ];

    if (last) {
      onsubmit?.(next);
      return;
    }
    given = next;
    index += 1;
    selected = '';
    values = [];
  }
</script>

<div class="question">
  {#if questions.length > 1}
    <span class="progress">{index + 1} / {questions.length}</span>
  {/if}
  <span class="header">{question.header}</span>
  <p class="text">{question.question}</p>
  <OptionList
    options={toOptions(question)}
    multiple={question.multi_select}
    bind:selected
    bind:values
    onselect={() => {
      // A single choice is a complete answer to this question by itself:
      // clicking it reads the same as pressing Next. tech.md 6.14.
      if (!question.multi_select) advance();
    }}
  />
  <!-- Checked boxes need an explicit confirm: unlike a click, nothing about
       checking one says the user is done choosing. tech.md 6.14. -->
  {#if question.multi_select}
    <div class="actions">
      <Button
        label={last ? 'Submit' : 'Next'}
        variant="primary"
        disabled={!answered}
        onclick={advance}
        wide
      />
    </div>
  {/if}
</div>

<style>
  .question {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .progress {
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .header {
    color: var(--text-dim);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .text {
    margin: 0 0 4px;
    color: var(--text);
    font-size: 14px;
  }

  .actions {
    padding-top: 4px;
  }
</style>
