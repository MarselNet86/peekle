<script lang="ts">
  /**
   * One to four AskUserQuestion questions, answered one at a time. tech.md
   * 6.14.
   *
   * One at a time and not all at once: `OptionList` binds a global digit
   * shortcut to whatever is on screen, and two mounted together would answer
   * the same keystroke twice.
   */
  import { MessageSquare } from '@lucide/svelte';
  import { untrack } from 'svelte';

  import { CHAT } from '$lib/i18n/chat';
  import { copy } from '$lib/i18n/index.svelte';
  import type { ChoiceOption } from '$lib/types/generated/ChoiceOption';
  import type { Question } from '$lib/types/generated/Question';
  import type { QuestionAnswer } from '$lib/types/generated/QuestionAnswer';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
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
    choice = null,
    onsubmit,
    onclose,
  }: {
    questions: Question[];
    /** The last ⌘ and digit pressed in another application while this stood,
     * numbered so two presses of one row are two presses. A new number picks
     * that row of the question on screen; the one standing when this mounted
     * picks nothing. tech.md 6.14. */
    choice?: { index: number; seq: number } | null;
    onsubmit?: (answers: QuestionAnswer[]) => void;
    /** None of these, and not a later one either. The tool is refused and the
     * agent moves on, rather than being handed the question again in the
     * terminal. tech.md 6.14. */
    onclose?: () => void;
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
      { id: OWN, label: t.other, hint: t.writeOwn, kind: 'Custom' },
    ];
  }

  const t = $derived(copy(CHAT));

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

  let list = $state<{ pick: (index: number) => boolean } | null>(null);
  let seen = untrack(() => choice?.seq ?? 0);

  $effect(() => {
    const next = choice;
    if (!next || next.seq === seen) return;
    seen = next.seq;
    untrack(() => list?.pick(next.index));
  });

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
    <!-- Who is asking, in the brand green, and the question's own short label
         beside it. tech.md 6.14. -->
    <span class="asks">
      <MessageSquare size={14} strokeWidth={2.2} aria-hidden="true" />
      {t.claudeAsks}
    </span>
    <span class="header">{question.header}</span>
    {#if questions.length > 1}
      <span class="progress">{index + 1} / {questions.length}</span>
    {/if}
    <!-- A way out of the question itself, not just out of this one of its
         answers. Without it the only way past a question with nothing right
         in it is to answer it wrongly. tech.md 6.14. -->
    {#if onclose}
      <IconButton name="close" title={t.closeQuestion} onclick={onclose} />
    {/if}
  </div>
  <p class="text">{question.question}</p>
  <!-- Pulled out by the row's own padding, so the answers line up with the
       question above them instead of sitting in from it. -->
  <div class="options">
    <OptionList
      bind:this={list}
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
    <PromptInput compact bind:value={own} placeholder={t.typeAnswer} onsubmit={() => advance()} />
  {/if}
  <!-- Checked boxes need an explicit confirm: unlike a click, nothing about
       checking one says the user is done choosing. tech.md 6.14. -->
  {#if question.multi_select}
    <div class="actions">
      <Button
        label={last ? t.submit : t.next}
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
    gap: 10px;
    min-width: 0;
  }

  .asks {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--brand);
    font-size: 13px;
    font-weight: 600;
  }

  .head {
    display: flex;
    /* The cross is a round button and the header a small caps line, so they
       are centred against each other rather than sat on one baseline. */
    align-items: center;
    gap: 8px;
  }

  .progress {
    flex: none;
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  /* The header takes the room and the cross keeps to its end, so a question
     with one page and a question with four look the same at the corner. */
  .header {
    flex: 1;
    min-width: 0;
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
    font-size: 15px;
    line-height: 21px;
  }

  /* The rows are cards with their own edge now, so they line up with the
     question rather than bleeding past it. */
  .options {
    margin: 0;
  }

  .actions {
    padding-top: 2px;
  }
</style>
