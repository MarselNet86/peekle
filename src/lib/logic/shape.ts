/**
 * Island geometry. Pure, so the property tests can hammer it.
 *
 * The window never resizes (tech.md 6.7), so every shape has to fit inside the
 * bounds below. Anything wider or taller would be clipped by the webview with
 * no error, which is exactly the failure that is hard to see in a screenshot.
 */

import type { IslandView } from '$lib/types/generated/IslandView';

export const WINDOW = { width: 720, height: 560 } as const;

/** Stand-in for a display with no notch to measure. */
export const FALLBACK_NOTCH = { width: 200, height: 0 } as const;

export interface Notch {
  width: number;
  height: number;
}

export interface ShapeBounds {
  width: number;
  height: number;
  /** Bottom corners. The top is cut by the screen edge. tech.md 6.10. */
  radius: number;
}

/** Rust hands these over in the query string and either can be absent. */
export function readNotch(search: string): Notch {
  const params = new URLSearchParams(search);
  return {
    width: finite(params.get('notch_width'), FALLBACK_NOTCH.width),
    height: finite(params.get('notch'), FALLBACK_NOTCH.height),
  };
}

function finite(raw: string | null, fallback: number): number {
  const value = Number(raw);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function sane(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

/**
 * Target bounds of the black shape for a view. Collapsed returns the notch
 * itself rather than zero: the spring has to grow out of the bezel, and a
 * shape that starts at nothing reads as a window appearing, not as the notch
 * opening.
 */
export function shapeBounds(view: IslandView, notch: Notch): ShapeBounds {
  const width = sane(notch.width, FALLBACK_NOTCH.width);
  const height = sane(notch.height, FALLBACK_NOTCH.height);

  const bounds =
    view === 'Collapsed'
      ? { width, height, radius: 0 }
      : view === 'Pill'
        ? { width: 420, height: height + 44, radius: 20 }
        : view === 'Sessions'
          ? { width: 460, height: height + 300, radius: 24 }
          : { width: 560, height: height + 420, radius: 24 };

  return clamp(bounds);
}

function clamp(bounds: ShapeBounds): ShapeBounds {
  const width = Math.min(Math.max(bounds.width, 0), WINDOW.width);
  const height = Math.min(Math.max(bounds.height, 0), WINDOW.height);
  return {
    width,
    height,
    radius: Math.min(Math.max(bounds.radius, 0), width / 2, height / 2),
  };
}
