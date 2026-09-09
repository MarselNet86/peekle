/**
 * S18 acceptance. The row under the field shows what the session answers with
 * and changes it, and it changes nothing at all in a session the island
 * cannot type into. tech.md 6.15.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import {
  contextLabel,
  modeLabel,
  MODE_NOTE,
  currentModel,
  effortOptions,
  modelLabel,
  noteTitle,
  settingsNote,
  askedLabel,
  ELSEWHERE_NOTE,
  FINISHED_NOTE,
} from '$lib/logic/agent';
import AgentBar from '$lib/ui/AgentBar.svelte';
import ContextRing from '$lib/ui/ContextRing.svelte';
import UsageCorner from '$lib/ui/UsageCorner.svelte';
import PickerMenu from '$lib/ui/PickerMenu.svelte';
import type { AgentSetup } from '$lib/types/generated/AgentSetup';
import type { ModelChoice } from '$lib/types/generated/ModelChoice';

const models: ModelChoice[] = [
  { alias: 'fable', label: 'Fable 5', id: 'claude-fable-5' },
  { alias: 'opus', label: 'Opus 5', id: 'claude-opus-5' },
  { alias: 'sonnet', label: 'Sonnet 5', id: 'claude-sonnet-5' },
  { alias: 'haiku', label: 'Haiku 4.5', id: 'claude-haiku-4-5' },
];

const opus: AgentSetup = {
  model: 'claude-opus-5',
  label: 'Opus 5',
  effort: 'High',
  levels: ['Low', 'Medium', 'High', 'XHigh', 'Max'],
  context_tokens: 612_000,
  context_window: 1_000_000,
  context_pct: 61.2,
};

const haiku: AgentSetup = {
  model: 'claude-haiku-4-5',
  label: 'Haiku 4.5',
  effort: null,
  levels: [],
  context_tokens: 24_000,
  context_window: 200_000,
  context_pct: 12,
};

describe('the block of a live session', () => {
  const block = () => screen.getByRole('button', { name: /Opus 5/ });

  it('names the model and the weight beside it', () => {
    render(AgentBar, { props: { agent: opus, models, live: true } });

    expect(block()).toHaveTextContent('Opus 5');
    expect(block()).toHaveTextContent('High');
  });

  it('sends the alias of the model that was picked, not its label', async () => {
    const onmodel = vi.fn();
    render(AgentBar, { props: { agent: opus, models, live: true, onmodel } });

    await userEvent.click(block());
    await userEvent.click(screen.getByRole('menuitemradio', { name: /Sonnet 5/ }));

    expect(onmodel).toHaveBeenCalledExactlyOnceWith('sonnet');
  });

  /** The effort is a track at the foot of the same menu, the way the
   * original draws it: one stop per level. tech.md 6.15. */
  it('sends the level whose stop was pressed', async () => {
    const oneffort = vi.fn();
    render(AgentBar, { props: { agent: opus, models, live: true, oneffort } });

    await userEvent.click(block());
    await userEvent.click(screen.getByRole('button', { name: 'Max' }));

    expect(oneffort).toHaveBeenCalledExactlyOnceWith('Max');
  });

  /** `--effort` does not take ultracode and `/effort ultracode` does, and it
   * holds for a running session only. So it is a stop past the end of the
   * track, and only there is one. tech.md 6.15. */
  it('offers ultracode past the last level, and only to a running session', async () => {
    const onultra = vi.fn();
    const { unmount } = render(AgentBar, {
      props: { agent: opus, models, live: true, canUltra: true, onultra },
    });

    await userEvent.click(block());
    await userEvent.click(screen.getByRole('button', { name: 'Ultracode' }));
    expect(onultra).toHaveBeenCalledOnce();
    unmount();

    render(AgentBar, { props: { agent: opus, models, live: true, canUltra: false } });
    await userEvent.click(block());
    expect(screen.queryByRole('button', { name: 'Ultracode' })).not.toBeInTheDocument();
  });

  /** Nothing reports ultracode back -- the file records the xhigh it runs at
   * -- so the block says it itself, in the colour the original uses. */
  it('wears ultracode on the block', () => {
    const { container } = render(AgentBar, {
      props: { agent: opus, models, live: true, ultra: true, canUltra: true },
    });

    expect(container.querySelector('.chip')).toHaveClass('ultra');
    expect(screen.getByRole('button', { name: /Ultracode/ })).toBeInTheDocument();
  });

  /** Nothing confirms a write to a pty, so a pick stands dimmed until the
   * transcript names it back. tech.md 6.15. */
  it('marks a pick that the agent has not confirmed', () => {
    const { container } = render(AgentBar, {
      props: { agent: opus, models, live: true, pendingModel: true },
    });

    expect(container.querySelector('.name')).toHaveClass('pending');
  });

  /** The ring is back where the original keeps it: the far end of this row,
   * furthest from send, because it acts on what is already spent. */
  it('carries the context ring, and pressing it compacts', async () => {
    const oncompact = vi.fn();
    render(AgentBar, {
      props: { agent: opus, models, live: true, contextTitle: 'ring', oncompact },
    });

    await userEvent.click(screen.getByRole('button', { name: 'ring' }));
    expect(oncompact).toHaveBeenCalledOnce();
  });

  /** Haiku takes no effort at all, so it is offered no track. */
  it('draws no effort track for a model that takes none', async () => {
    render(AgentBar, { props: { agent: haiku, models, live: true } });

    await userEvent.click(screen.getByRole('button', { name: /Haiku 4.5/ }));
    expect(screen.queryByRole('button', { name: 'Max' })).not.toBeInTheDocument();
  });

  /** Set on the spawn and reported by nothing, so it is offered before a
   * session runs and read after. tech.md 6.20. */
  it('switches thinking before the session runs', async () => {
    const onthinking = vi.fn();
    render(AgentBar, {
      props: { agent: opus, models, live: true, canSetThinking: true, thinking: true, onthinking },
    });

    await userEvent.click(screen.getByRole('button', { name: 'Thinking' }));
    await userEvent.click(screen.getByRole('switch', { name: /Thinking/ }));
    expect(onthinking).toHaveBeenCalledExactlyOnceWith(false);
  });

  it('says when thinking is off without being opened', () => {
    render(AgentBar, { props: { agent: opus, models, live: true, thinking: false } });

    expect(screen.getByRole('button', { name: /Thinking/ })).toHaveTextContent('off');
  });
});

/**
 * The corner keeps the two windows. The context ring went back to the row
 * under the field in v65, and took its one action with it. tech.md 6.15.
 */
describe('UsageCorner', () => {
  it('carries the two windows and nothing else', () => {
    const { container } = render(UsageCorner, { props: { hour: 42, week: 68 } });

    expect(screen.getByText('42% 5h')).toBeInTheDocument();
    expect(screen.getByText('68% 7d')).toBeInTheDocument();
    expect(container.querySelector('button')).toBeNull();
  });
});

describe('ContextRing', () => {
  it('says what it shows and what pressing it does', () => {
    render(ContextRing, {
      props: { pct: 56, title: '56% of context used. Click to compact.', live: true },
    });

    expect(
      screen.getByRole('button', { name: '56% of context used. Click to compact.' }),
    ).toBeInTheDocument();
  });

  it('compacts when pressed', async () => {
    const onclick = vi.fn();
    render(ContextRing, { props: { pct: 56, title: 'ring', live: true, onclick } });

    await userEvent.click(screen.getByRole('button', { name: 'ring' }));
    expect(onclick).toHaveBeenCalledOnce();
  });

  /** Nothing has been asked of the window, and an empty ring says that. A
   * drawn zero would be a measurement nobody made. */
  it('refuses to compact before the first answer', async () => {
    const onclick = vi.fn();
    render(ContextRing, { props: { pct: null, title: 'ring', live: true, onclick } });

    const ring = screen.getByRole('button', { name: 'ring' });
    expect(ring).toBeDisabled();
    await userEvent.click(ring);
    expect(onclick).not.toHaveBeenCalled();
  });

  it('waits visibly while a compact travels', () => {
    const { container } = render(ContextRing, {
      props: { pct: 56, title: 'ring', live: true, pending: true },
    });

    expect(container.querySelector('.ring')).toHaveClass('pending');
  });
});

describe('a session that has not answered yet', () => {
  /** A model is chosen before the first turn, not after it, so the row stands
   * on Claude Code's own defaults until the transcript names one. 6.15. */
  const defaults: AgentSetup = {
    model: 'claude-opus-5',
    label: 'Opus 5',
    effort: 'Low',
    levels: ['Low', 'Medium', 'High', 'XHigh', 'Max'],
    context_tokens: 0,
    context_window: 1_000_000,
    context_pct: 0,
  };

  it('can be aimed before it speaks', async () => {
    const onmodel = vi.fn();
    render(AgentBar, { props: { agent: null, defaults, models, live: true, onmodel } });

    await userEvent.click(screen.getByRole('button', { name: /Opus 5/ }));
    await userEvent.click(screen.getByRole('menuitemradio', { name: /Haiku 4.5/ }));

    expect(onmodel).toHaveBeenCalledExactlyOnceWith('haiku');
  });

  /**
   * A pick shows as made the moment it is made. The old value staying on
   * screen read as a menu that did nothing, and on a session that has not
   * answered yet it stayed there until the first message went. tech.md 6.15.
   */
  it('stands on the value asked for while it travels', () => {
    render(AgentBar, {
      props: {
        agent: opus,
        models,
        live: true,
        askedModel: 'sonnet',
        askedEffort: 'Low' as const,
        pendingModel: true,
        pendingEffort: true,
      },
    });

    // The block stands on what was asked for, both halves of it.
    const block = screen.getByRole('button', { name: /Sonnet 5/ });
    expect(block).toHaveTextContent('Sonnet 5');
    expect(block).toHaveTextContent('Low');
    expect(block).not.toHaveTextContent('Opus 5');
  });

  /** A pick the catalog has never heard of shows by its own alias. */
  it('names an unknown pick by its alias rather than by nothing', () => {
    expect(askedLabel('sonnet', models)).toBe('Sonnet 5');
    expect(askedLabel('tomorrow', models)).toBe('tomorrow');
    expect(askedLabel(null, models)).toBe('');
  });

  /** Settings that name no model still leave a working picker: it is for
   * choosing one, not for confirming one. */
  it('offers the picker even when nothing is known', async () => {
    const onmodel = vi.fn();
    const unknown: AgentSetup = { ...defaults, model: null, label: null, effort: null };
    render(AgentBar, { props: { agent: null, defaults: unknown, models, live: true, onmodel } });

    await userEvent.click(screen.getByRole('button', { name: 'Model' }));
    await userEvent.click(screen.getByRole('menuitemradio', { name: /Opus 5/ }));
    expect(onmodel).toHaveBeenCalledExactlyOnceWith('opus');
  });
});

describe('the row of a session the island cannot command', () => {
  /** A chat another app runs takes text but not commands (6.5), so the row
   * reads. The chevron is what says so on sight: without it there is nothing
   * to tell a value that opens a menu from a value that does not, and the
   * reader has to press to find out. tech.md 6.15. */
  it('wears no chevron, so it is not read as a menu', async () => {
    const handlers = { onmodel: vi.fn(), oneffort: vi.fn() };
    const { container } = render(AgentBar, {
      props: { agent: opus, models, live: false, note: noteTitle(ELSEWHERE_NOTE), ...handlers },
    });

    expect(container.querySelectorAll('.chev')).toHaveLength(0);
    for (const name of ['Opus 5', 'High'] as const) {
      expect(screen.getByText(name)).toBeTruthy();
    }
    // The block carries the note, so the reader who looks at it is told where
    // the setting lives.
    expect(screen.getAllByTitle(noteTitle(ELSEWHERE_NOTE)).length).toBeGreaterThan(0);

    // Pressed, it changes nothing and opens nothing, whatever else it does.
    await userEvent.click(screen.getByText('Opus 5'));
    expect(handlers.onmodel).not.toHaveBeenCalled();
    expect(handlers.oneffort).not.toHaveBeenCalled();
    expect(screen.queryByRole('menuitemradio')).not.toBeInTheDocument();
  });

  /** No chevrons anywhere: a triangle at eight pixels was noise, and what
   * separates a value that opens a menu from one that only reads is that the
   * first is lit and takes a hover. tech.md 9. */
  it('carries no chevrons, and reads quietly where it cannot be changed', () => {
    const live = render(AgentBar, {
      props: { agent: opus, models, live: true, canPickMode: true },
    });
    expect(live.container.querySelectorAll('.chev')).toHaveLength(0);
    expect(live.container.querySelectorAll('.reading')).toHaveLength(0);
    live.unmount();

    const reading = render(AgentBar, { props: { agent: opus, models, live: false } });
    expect(reading.container.querySelectorAll('.chev')).toHaveLength(0);
    expect(reading.container.querySelectorAll('.reading').length).toBeGreaterThan(0);
  });

  /** A value that looks like a value gets pressed anyway. Silence is the
   * worst answer to that, so the press asks the question and the note
   * answers it. tech.md 6.15. */
  it('answers a press with the reason instead of nothing', async () => {
    const onnote = vi.fn();
    render(AgentBar, { props: { agent: opus, models, live: false, onnote } });

    await userEvent.click(screen.getByText('Opus 5'));
    await userEvent.click(screen.getByText('High'));
    expect(onnote).toHaveBeenCalledTimes(2);
  });

  /** With nothing to stand on -- no transcript, no defaults -- there is no
   * row. Zeroes would claim an empty context, which is a different claim. */
  it('draws nothing at all with neither a reading nor a default', () => {
    const { container } = render(AgentBar, {
      props: { agent: null, defaults: null, models, live: true },
    });
    expect(container.querySelector('.agent-bar')).toBeNull();
  });
});

describe('the menu', () => {
  const options = [
    { id: 'opus', label: 'Opus 5' },
    { id: 'sonnet', label: 'Sonnet 5' },
  ];

  it('picks a row by its digit', async () => {
    const onpick = vi.fn();
    render(PickerMenu, { props: { label: 'Opus 5', options, value: 'opus', onpick } });

    await userEvent.click(screen.getByRole('button', { name: 'Opus 5' }));
    await userEvent.keyboard('2');

    expect(onpick).toHaveBeenCalledExactlyOnceWith('sonnet');
  });

  it('closes on Escape without picking anything', async () => {
    const onpick = vi.fn();
    render(PickerMenu, { props: { label: 'Opus 5', options, onpick } });

    await userEvent.click(screen.getByRole('button', { name: 'Opus 5' }));
    expect(screen.getByRole('menu')).toBeInTheDocument();

    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(onpick).not.toHaveBeenCalled();
  });

  it('marks the row that is in force', async () => {
    render(PickerMenu, { props: { label: 'Opus 5', options, value: 'opus', onpick: vi.fn() } });

    await userEvent.click(screen.getByRole('button', { name: 'Opus 5' }));
    expect(screen.getByRole('menuitemradio', { name: /Opus 5/ })).toBeChecked();
    expect(screen.getByRole('menuitemradio', { name: /Sonnet 5/ })).not.toBeChecked();
  });
});

describe('what the row says', () => {
  /** The transcript can write a dated first party id the catalog key lacks. */
  it('finds the current model behind a dated id', () => {
    expect(currentModel({ ...haiku, model: 'claude-haiku-4-5-20251001' }, models)).toBe('haiku');
    expect(currentModel(opus, models)).toBe('opus');
    expect(currentModel({ ...opus, model: 'claude-tomorrow-9' }, models)).toBe('');
    expect(currentModel(null, models)).toBe('');
  });

  /** A model the catalog has never heard of is shown by its own id: a session
   * that answers with something is never described by nothing. */
  it('falls back to the id of an unknown model', () => {
    expect(modelLabel({ ...opus, label: null, model: 'claude-tomorrow-9' })).toBe(
      'claude-tomorrow-9',
    );
    expect(modelLabel(null)).toBe('');
  });

  it('says what the ring is showing and what clicking it does', () => {
    expect(contextLabel(opus)).toBe('61% of context used, 612k of 1000k. Click to compact.');
  });

  /** A ring that cannot be compacted still reports the number: the reading is
   * true whoever is driving the session. Only the invitation goes. 6.15. */
  it('drops the invitation when there is nothing to click', () => {
    expect(contextLabel(opus, false)).toBe('61% of context used, 612k of 1000k.');
    expect(contextLabel(null, false)).toBe('');
  });

  /**
   * S21 left a chat another app runs as an observed one, so its row reads and
   * does not change: a slash command cannot ride the inbox, which wraps what
   * it carries. The row says where the setting lives instead of dimming and
   * leaving the reason to be guessed at. tech.md 6.15.
   */
  describe('why the row only reads', () => {
    it('sends the reader to the app that runs the chat', () => {
      expect(settingsNote({ origin: 'Observed', status: 'Working' })).toBe(ELSEWHERE_NOTE);
      expect(settingsNote({ origin: 'Observed', status: 'Idle' })).toBe(ELSEWHERE_NOTE);
    });

    /** The wall has a door, and the note names it: the moment the other app
     * lets go, the next message continues the chat here and the row comes
     * alive with it (6.5). A reason with no way out is half an answer. */
    it('names the way out rather than only the reason', () => {
      expect(ELSEWHERE_NOTE.how).toMatch(/Close it there/);
      expect(FINISHED_NOTE.how).toBeUndefined();
      // Two halves read as two thoughts; one long line reads as an error.
      expect(noteTitle(ELSEWHERE_NOTE)).toBe(`${ELSEWHERE_NOTE.fact} ${ELSEWHERE_NOTE.how}`);
      expect(noteTitle(null)).toBe('');
      expect(noteTitle(FINISHED_NOTE)).toBe(FINISHED_NOTE.fact);
    });

    it('says a finished session is finished rather than blaming another app', () => {
      expect(settingsNote({ origin: 'Owned', status: 'Ended' })).toBe(FINISHED_NOTE);
    });

    it('says nothing at all when the row changes things', () => {
      expect(settingsNote({ origin: 'Owned', status: 'Working' })).toBeNull();
      expect(settingsNote({ origin: 'Owned', status: 'Idle' })).toBeNull();
      expect(settingsNote(null)).toBeNull();
    });
  });

  it('offers no levels for a model that takes none', () => {
    expect(effortOptions(haiku)).toEqual([]);
    expect(effortOptions(opus).map((option) => option.id)).toEqual(opus.levels);
  });
});

/**
 * v63 acceptance. The mode is Claude Code's own, by its own names, read from
 * the hooks that carry it and written only where the CLI takes it: the flag
 * on a session Peekle starts. tech.md 6.19.
 */
describe('the permission mode', () => {
  it('offers the four modes to a session that has not started', async () => {
    const onmode = vi.fn();
    render(AgentBar, {
      props: { agent: null, defaults: opus, models, live: true, canPickMode: true, onmode },
    });

    await userEvent.click(screen.getByRole('button', { name: 'Manual' }));
    for (const label of ['Manual', 'Edit automatically', 'Plan', 'Auto']) {
      expect(screen.getByRole('menuitemradio', { name: new RegExp(label) })).toBeInTheDocument();
    }

    await userEvent.click(screen.getByRole('menuitemradio', { name: /Plan/ }));
    expect(onmode).toHaveBeenCalledExactlyOnceWith('Plan');
  });

  /** Never offered: a mode that asks for nothing is not handed over in a
   * menu. A session already in one still reads as it. */
  it('offers neither of the two that ask for nothing', async () => {
    render(AgentBar, {
      props: { agent: null, defaults: opus, models, live: true, canPickMode: true },
    });

    await userEvent.click(screen.getByRole('button', { name: 'Manual' }));
    expect(screen.queryByRole('menuitemradio', { name: /No permissions/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('menuitemradio', { name: /Never asks/ })).not.toBeInTheDocument();
    expect(modeLabel('Bypass')).toBe('No permissions');
  });

  it('stands on what the session reported, not on a guess', () => {
    render(AgentBar, { props: { agent: opus, models, live: true, mode: 'Auto' } });

    expect(screen.getByRole('button', { name: 'Auto' })).toBeInTheDocument();
  });

  /** The flag is a spawn flag and the CLI has no line for the mode, so a
   * session under way reads and says where the switch is. */
  it('reads once the session has answered, and answers a press with the note', async () => {
    const onmodenote = vi.fn();
    render(AgentBar, {
      props: { agent: opus, models, live: true, mode: 'Plan', canPickMode: false, onmodenote },
    });

    const chip = screen.getByRole('button', { name: 'Plan' });
    expect(chip).toHaveAttribute('title', noteTitle(MODE_NOTE));
    await userEvent.click(chip);
    expect(onmodenote).toHaveBeenCalledOnce();
  });

  /** A session that has never reported one is in the CLI's own default. */
  it('shows Manual when nothing has said otherwise', () => {
    render(AgentBar, { props: { agent: opus, models, live: true } });

    expect(screen.getByRole('button', { name: 'Manual' })).toBeInTheDocument();
  });
});
