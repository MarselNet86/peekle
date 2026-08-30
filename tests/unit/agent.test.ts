/**
 * S18 acceptance. The row under the field shows what the session answers with
 * and changes it, and it changes nothing at all in a session the island
 * cannot type into. tech.md 6.15.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { contextLabel, currentModel, effortOptions, modelLabel } from '$lib/logic/agent';
import AgentBar from '$lib/ui/AgentBar.svelte';
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

describe('the row of a live session', () => {
  it('names the model, the effort and the context', () => {
    render(AgentBar, { props: { agent: opus, models, live: true } });

    expect(screen.getByRole('button', { name: 'Opus 5' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'High' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /61% of context used/ })).toBeInTheDocument();
  });

  it('sends the alias of the model that was picked, not its label', async () => {
    const onmodel = vi.fn();
    render(AgentBar, { props: { agent: opus, models, live: true, onmodel } });

    await userEvent.click(screen.getByRole('button', { name: 'Opus 5' }));
    await userEvent.click(screen.getByRole('menuitemradio', { name: /Sonnet 5/ }));

    expect(onmodel).toHaveBeenCalledExactlyOnceWith('sonnet');
  });

  it('sends the level that was picked', async () => {
    const oneffort = vi.fn();
    render(AgentBar, { props: { agent: opus, models, live: true, oneffort } });

    await userEvent.click(screen.getByRole('button', { name: 'High' }));
    await userEvent.click(screen.getByRole('menuitemradio', { name: /Max/ }));

    expect(oneffort).toHaveBeenCalledExactlyOnceWith('Max');
  });

  /** The ring is the button, exactly as it is in Claude Code. */
  it('compacts when the ring is clicked', async () => {
    const oncompact = vi.fn();
    render(AgentBar, { props: { agent: opus, models, live: true, oncompact } });

    await userEvent.click(screen.getByRole('button', { name: /Click to compact/ }));
    expect(oncompact).toHaveBeenCalledOnce();
  });

  /** Nothing confirms a write to a pty, so a pick stands dimmed until the
   * transcript names it back. tech.md 6.15. */
  it('marks a pick that the agent has not confirmed', () => {
    render(AgentBar, {
      props: { agent: opus, models, live: true, pendingModel: true, pendingCompact: true },
    });

    expect(screen.getByRole('button', { name: 'Opus 5' })).toHaveClass('pending');
    expect(screen.getByRole('button', { name: /Click to compact/ })).toHaveClass('pending');
  });

  /** Haiku takes no effort at all, so it is not offered a dead menu. */
  it('offers no effort menu to a model that takes none', () => {
    render(AgentBar, { props: { agent: haiku, models, live: true } });

    expect(screen.getByRole('button', { name: 'Haiku 4.5' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Effort' })).not.toBeInTheDocument();
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

    await userEvent.click(screen.getByRole('button', { name: 'Opus 5' }));
    await userEvent.click(screen.getByRole('menuitemradio', { name: /Haiku 4.5/ }));

    expect(onmodel).toHaveBeenCalledExactlyOnceWith('haiku');
  });

  /** Nothing has been asked of the window, and an empty ring says that. A
   * drawn zero would be a measurement nobody made. */
  it('leaves the ring empty and refuses to compact', async () => {
    const oncompact = vi.fn();
    render(AgentBar, { props: { agent: null, defaults, models, live: true, oncompact } });

    const ring = screen.getByRole('button', { name: 'Nothing in the context yet' });
    expect(ring).toBeDisabled();
    await userEvent.click(ring);
    expect(oncompact).not.toHaveBeenCalled();
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

describe('the row of a session the island cannot type into', () => {
  it('reads, and calls nothing', async () => {
    const handlers = { onmodel: vi.fn(), oneffort: vi.fn(), oncompact: vi.fn() };
    render(AgentBar, { props: { agent: opus, models, live: false, ...handlers } });

    for (const name of ['Opus 5', 'High', /Click to compact/] as const) {
      await userEvent.click(screen.getByRole('button', { name }));
    }

    expect(screen.getByRole('button', { name: 'Opus 5' })).toBeDisabled();
    expect(handlers.onmodel).not.toHaveBeenCalled();
    expect(handlers.oneffort).not.toHaveBeenCalled();
    expect(handlers.oncompact).not.toHaveBeenCalled();
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

  it('offers no levels for a model that takes none', () => {
    expect(effortOptions(haiku)).toEqual([]);
    expect(effortOptions(opus).map((option) => option.id)).toEqual(opus.levels);
  });
});
