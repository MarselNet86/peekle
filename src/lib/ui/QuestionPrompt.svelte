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
  import PromptInput from './PromptInput.svelte';

  /**
   * The row that is not one of Claude's answers. AskUserQuestion always lets
   * a person write their own instead of picking, and a question without that
   * makes them choose the nearest thing that does not quite fit. tech.md
   * 6.14.
   *
   * Not a label: `answers` reports the label back, so the id has to be
   * something no answer of Claude's would ever be.
   */
  const OWN = '\u0000 peekle: your own answer';

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
    return [
      ...question.options.map((option): ChoiceOption => ({
        id: option.label,
        label: option.label,
        hint: option.description,
        kind: 'Custom',
      })),
      // Last, where an escape hatch belongs: what Claude offered is read
      // first, and this is what to do when none of it is the answer.
      { id: OWN, label: 'Other', hint: 'Write your own answer', kind: 'Custom' },
    ];
  }

  let index = $state(0);
  let selected = $state('');
  let values = $state<string[]>([]);
  let given: QuestionAnswer[] = $state([]);
  /** What was written instead of picked. */
  let own = $state('');

  const question = $derived(questions[index]);
  const last = $derived(index === questions.length - 1);
  /** The row for a written answer is picked, so the field is open. */
  const writing = $derived(question.multi_select ? values.includes(OWN) : selected === OWN);
  /** What the picked rows and the written line come to, in order, with the
   * row that only opens the field left out of it. */
  const labels = $derived.by(() => {
    const picked = (question.multi_select ? values : selected === '' ? [] : [selected]).filter(
      (label) => label !== OWN,
    );
    const written = own.trim();
    return writing && written !== '' ? [...picked, written] : picked;
  });
  const answered = $derived(labels.length > 0);

  /** Records this question's answer and moves to the next, or submits every
   * answer once the last one is in. */
  function advance() {
    if (!answered) return;

    const next = [...given, { question: question.question, labels }];

    if (last) {
      onsubmit?.(next);
      return;
    }
    given = next;
    index += 1;
    selected = '';
    values = [];
    own = '';
  }
</script>

<div class="question">
  <div class="head">
    <span class="header">{question.header}</span>
    {#if questions.length > 1}
      <span class="progress">{index + 1} / {questions.length}</span>
    {/if}
  </div>
  <p class="text">{question.question}</p>
  <!-- Pulled out by the row's own padding, so the answers line up with the
       question above them instead of sitting in from it. -->
  <div class="options">
    <OptionList
      options={toOptions(question)}
      multiple={question.multi_select}
      bind:selected
      bind:values
      onselect={(id) => {
        // A single choice is a complete answer to this question by itself:
        // clicking it reads the same as pressing Next. Picking the row that
        // opens the field is not an answer yet -- the answer is what gets
        // written in it. tech.md 6.14.
        if (!question.multi_select && id !== OWN) advance();
      }}
    />
  </div>
  <!-- Open only once the row above it is picked: a field standing under every
       question would read as the thing to do, when picking one of Claude's
       answers is. Enter sends it, and it sends the whole answer -- the boxes
       that are checked included -- because the field is the last thing being
       filled in and Enter in a field means done everywhere else in the
       product. tech.md 6.14. -->
  {#if writing}
    <PromptInput bind:value={own} placeholder="Your answer" onsubmit={() => advance()} />
  {/if}
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
    gap: 8px;
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .progress {
    flex: none;
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .header {
    color: var(--text-dim);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .text {
    margin: 0;
    color: var(--text);
    font-size: 14px;
    line-height: 20px;
  }

  .options {
    margin: 0 -10px;
  }

  .actions {
    padding-top: 2px;
  }
</style>
