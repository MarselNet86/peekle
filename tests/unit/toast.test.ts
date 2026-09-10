/**
 * The pill at the end of a turn, in the shape the system uses for the same
 * job: who it was, what they said under it, and how long it took at the end
 * of that line. tech.md 6.2 and 9.
 */

import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import Toast from '$lib/ui/Toast.svelte';

describe('the end of a turn', () => {
  it('says who finished, what they said, and how long it took', () => {
    render(Toast, {
      props: { text: 'peekle', detail: 'Готово: шапка переехала', tookMs: 72_000 },
    });

    expect(screen.getByText('peekle')).toBeInTheDocument();
    expect(screen.getByText('Готово: шапка переехала')).toBeInTheDocument();
    // The same words the work line uses for a span, because two readouts of
    // one duration must not disagree. tech.md 6.12.
    expect(screen.getByText('1m 12s')).toBeInTheDocument();
  });

  /// A turn nobody started in this chat has nothing to count from, and a
  /// made up number is worse than none. tech.md 6.2.
  it('carries no number when there is nothing to count', () => {
    const { container } = render(Toast, {
      props: { text: 'peekle', detail: 'Готово', tookMs: null },
    });

    expect(container.querySelector('.took')).toBeNull();
    expect(screen.getByText('Готово')).toBeInTheDocument();
  });

  it('reads a duration that could not be measured as none at all', () => {
    for (const tookMs of [0, -5, Number.NaN, Number.POSITIVE_INFINITY]) {
      const { container, unmount } = render(Toast, {
        props: { text: 'peekle', detail: 'Готово', tookMs },
      });

      expect(container.querySelector('.took'), `${tookMs}`).toBeNull();
      unmount();
    }
  });
});

describe('a pill with nothing to say twice', () => {
  /// A switch flipping is one line and stays one line: there is no second
  /// thing to put under `Peekle is ON`.
  it('draws one line and keeps its weight', () => {
    const { container } = render(Toast, { props: { text: 'Peekle is ON', tone: 'On' } });

    expect(container.querySelector('.under')).toBeNull();
    expect(container.querySelector('.band')).not.toHaveClass('deep');
  });

  it('still carries a count when it has one', () => {
    render(Toast, { props: { text: 'Claude needs your input', badge: 3 } });

    expect(screen.getByText('3')).toBeInTheDocument();
  });
});
