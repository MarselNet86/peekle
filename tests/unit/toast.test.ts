/**
 * The pill at the end of a turn, laid out the way the permission panel lays
 * out its own two lines: a sign at the head of the band, who it was and what
 * they said in the middle, and the one number a finished turn has at the
 * tail. tech.md 6.2, 6.7 and 9.
 */

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

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

    expect(container.querySelector('.detail')).toBeNull();
    expect(container.querySelector('.band')).not.toHaveClass('deep');
    // Nothing to put at the tail either: a switch flipping took no time.
    expect(container.querySelector('.trail')).toBeNull();
  });

  it('still carries a count when it has one', () => {
    render(Toast, { props: { text: 'Claude needs your input', badge: 3 } });

    expect(screen.getByText('3')).toBeInTheDocument();
  });
});

describe('the band it is drawn as', () => {
  /// Every pill is Peekle speaking, so every pill wears the sign at its head,
  /// the way a notification on this system wears the icon of whatever raised
  /// it. A bare dot said that in a language nobody reads. tech.md 9.
  it('wears the sign at its head in every tone', () => {
    for (const tone of ['Neutral', 'On', 'Off', 'Warn'] as const) {
      const { container, unmount } = render(Toast, { props: { text: 'peekle', tone } });

      const mark = container.querySelector('.mark svg');
      expect(mark, tone).not.toBeNull();
      expect(container.querySelector('.band')?.getAttribute('data-tone')).toBe(tone);
      unmount();
    }
  });

  /// The number stands at the tail, where the panel stands its buttons, and
  /// not as a footnote to a line that was already being clipped.
  it('stands the number at the tail rather than after the words', () => {
    const { container } = render(Toast, {
      props: { text: 'work', detail: 'Hi', tookMs: 7_400 },
    });

    expect(container.querySelector('.trail .took')?.textContent).toBe('7s');
    expect(container.querySelector('.detail')?.textContent).toBe('Hi');
  });

  /// The pill says how long it stands the way the panel says how long it
  /// waits: a hairline that leaks. tech.md 6.7.
  it('leaks for exactly as long as it stands', () => {
    const { container } = render(Toast, {
      props: { text: 'work', detail: 'Hi', ttlMs: 4_500 },
    });

    const leak = container.querySelector('.leak');
    expect(leak).not.toBeNull();
    expect((leak as HTMLElement).style.getPropertyValue('--secs')).toBe('4.5s');
  });

  it('draws no hairline when nobody said how long it stands', () => {
    for (const ttlMs of [null, 0]) {
      const { container, unmount } = render(Toast, { props: { text: 'work', ttlMs } });
      expect(container.querySelector('.leak'), `${ttlMs}`).toBeNull();
      unmount();
    }
  });
});

describe('the way into the chat it is about', () => {
  /// A notice names a place, and getting there has to cost one press rather
  /// than a walk through the list. The permission panel's body has always
  /// worked this way. tech.md 6.2 and 6.7.
  it('opens the chat when it is pressed', async () => {
    const onopen = vi.fn();
    render(Toast, { props: { text: 'work', detail: 'Hi', onopen } });

    await userEvent.click(screen.getByRole('button', { name: 'Open work' }));
    expect(onopen).toHaveBeenCalledOnce();
  });

  /// A pill about the product itself leads nowhere, and a control that leads
  /// nowhere lies about being one.
  it('is not a control at all when there is nowhere to go', () => {
    const { container } = render(Toast, { props: { text: 'Peekle is ON', tone: 'On' } });

    expect(container.querySelector('[role="button"]')).toBeNull();
    expect(container.querySelector('.band')).not.toHaveClass('pressable');
  });
});
