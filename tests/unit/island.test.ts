/**
 * Component tests for the island shape. The acceptance criterion of S1 is that
 * the shape carries the whole transition, so these check what the shape does
 * per view rather than what the spring does per frame.
 */

import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';

import Shape from '$lib/ui/Shape.svelte';
import { FLOAT_TOP, WINDOW } from '$lib/logic/shape';
import type { IslandView } from '$lib/types/generated/IslandView';

const notch = { width: 200, height: 32 };

/** jsdom keeps the shorthand verbatim, so read the corners off the tokens. */
function topCornersAreSquare(shape: HTMLElement): boolean {
  const corners = shape.style.borderRadius.trim().split(/\s+/);
  return corners.length === 4 && parseFloat(corners[0]) === 0 && parseFloat(corners[1]) === 0;
}

function shapeOf(container: HTMLElement): HTMLElement {
  const shape = container.querySelector('.shape');
  if (!(shape instanceof HTMLElement)) throw new Error('the shape did not render');
  return shape;
}

describe('Shape', () => {
  it('starts collapsed and invisible, so the notch reads as a plain notch', () => {
    const { container } = render(Shape, { props: { view: 'Collapsed' as IslandView, notch } });
    const shape = shapeOf(container);

    expect(shape).toHaveClass('collapsed');
    expect(shape.dataset.view).toBe('Collapsed');
  });

  it('names the view it is drawing, including a session by id', async () => {
    const { container, rerender } = render(Shape, {
      props: { view: 'Collapsed' as IslandView, notch },
    });

    for (const [view, name] of [
      ['Pill', 'Pill'],
      ['Sessions', 'Sessions'],
      [{ Session: 'abc' }, 'Session'],
      ['Collapsed', 'Collapsed'],
    ] as [IslandView, string][]) {
      await rerender({ view, notch });
      expect(shapeOf(container).dataset.view).toBe(name);
    }
  });

  it('drops the collapsed class the moment a view opens and takes it back', async () => {
    const { container, rerender } = render(Shape, {
      props: { view: 'Collapsed' as IslandView, notch },
    });

    await rerender({ view: 'Sessions' as IslandView, notch });
    expect(shapeOf(container)).not.toHaveClass('collapsed');

    await rerender({ view: 'Collapsed' as IslandView, notch });
    expect(shapeOf(container)).toHaveClass('collapsed');
  });

  it('never asks for a box bigger than the window', async () => {
    const { container, rerender } = render(Shape, {
      props: { view: 'Collapsed' as IslandView, notch },
    });

    for (const view of ['Pill', 'Sessions', { Session: 'abc' }] as IslandView[]) {
      await rerender({ view, notch });
      const shape = shapeOf(container);
      expect(parseFloat(shape.style.width)).toBeLessThanOrEqual(WINDOW.width);
      expect(parseFloat(shape.style.height)).toBeLessThanOrEqual(WINDOW.height);
    }
  });

  it('squares the top corners while a notch is on screen and rounds them without one', async () => {
    const { container, rerender } = render(Shape, { props: { view: 'Pill' as IslandView, notch } });
    expect(topCornersAreSquare(shapeOf(container))).toBe(true);

    await rerender({ view: 'Pill' as IslandView, notch: { width: 200, height: 0 } });
    expect(topCornersAreSquare(shapeOf(container))).toBe(false);
  });
});

describe('Shape off the edge', () => {
  /// Under a notch the black continues the cutout and sits flush with the
  /// edge; without one the shape floats, the way the iPhone island does.
  /// tech.md 6.7.
  it('sits flush under a notch and off the edge without one', async () => {
    const { container, rerender } = render(Shape, { props: { view: 'Pill' as IslandView, notch } });
    expect(parseFloat(shapeOf(container).style.marginTop)).toBe(0);

    await rerender({ view: 'Pill' as IslandView, notch: { width: 200, height: 0 } });
    expect(parseFloat(shapeOf(container).style.marginTop)).toBe(FLOAT_TOP);
  });
});

describe('Shape with content', () => {
  const label = createRawSnippet(() => ({ render: () => '<span>working</span>' }));

  it('renders what it is given, clear of the camera housing', () => {
    const { container } = render(Shape, {
      props: { view: 'Pill' as IslandView, notch, children: label },
    });

    expect(screen.getByText('working')).toBeInTheDocument();
    const content = container.querySelector('.content');
    expect(content).toBeInstanceOf(HTMLElement);
    expect((content as HTMLElement).style.paddingTop).toBe('32px');
  });

  /// The cutout is a hole in the middle of the top edge, and the pixels
  /// beside it are real screen. The shape hands both numbers to whatever it
  /// draws, so a view can put its top row up there rather than leaving the
  /// band black. tech.md 6.7.
  it('hands out the band beside the cutout', () => {
    const { container } = render(Shape, {
      props: { view: 'Pill' as IslandView, notch, children: label },
    });

    const content = container.querySelector('.content') as HTMLElement;
    expect(content.style.getPropertyValue('--notch-h')).toBe('32px');
    expect(content.style.getPropertyValue('--notch-w')).toBe('200px');
  });

  /// No cutout, no band and nothing to step around: a row that reads these
  /// stands exactly where it would have stood anyway.
  it('hands out nothing on a display with no notch', () => {
    const { container } = render(Shape, {
      props: {
        view: 'Pill' as IslandView,
        notch: { width: 200, height: 0 },
        children: label,
      },
    });

    const content = container.querySelector('.content') as HTMLElement;
    expect(content.style.getPropertyValue('--notch-h')).toBe('0px');
    expect(content.style.getPropertyValue('--notch-w')).toBe('0px');
  });

  it('holds the content back until the shape has moved, then fades it in', async () => {
    const { container, rerender } = render(Shape, {
      props: { view: 'Collapsed' as IslandView, notch, children: label },
    });

    await rerender({ view: 'Pill' as IslandView, notch, children: label });
    expect(container.querySelector('.content')).not.toHaveClass('shown');

    await new Promise((resolve) => setTimeout(resolve, 120));
    expect(container.querySelector('.content')).toHaveClass('shown');
  });

  it('takes the content away the instant the island collapses', async () => {
    const { container, rerender } = render(Shape, {
      props: { view: 'Pill' as IslandView, notch, children: label },
    });
    await new Promise((resolve) => setTimeout(resolve, 120));
    expect(container.querySelector('.content')).toHaveClass('shown');

    await rerender({ view: 'Collapsed' as IslandView, notch, children: label });
    expect(container.querySelector('.content')).not.toHaveClass('shown');
  });
});
