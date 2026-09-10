/**
 * The Peekle sign, as geometry. Two strokes leaning right, the way the CLI
 * writes `//`. tech.md 9.
 *
 * It lives here rather than inside the one component that used to draw it,
 * because it is drawn in more than one place now -- the resting mark, and the
 * dialogue that has nothing in it yet -- and a sign redrawn by hand in the
 * second place is a sign that drifts from the first. The app icon keeps its
 * own copy in `src-tauri/icons/icon.svg` for the same reason it keeps its own
 * pipeline: nothing in the webview can be rasterised into an `.icns`.
 */

/** The box both strokes are drawn in. Everything below is in its units. */
export const SIGN_BOX = { width: 14, height: 12 } as const;

/** The two strokes, first then second, in the order they hop and wave in. */
export const SIGN_STROKES = ['M4 10.6L6.9 1.4', 'M9.1 10.6L12 1.4'] as const;

/** How thick a stroke is drawn at box scale. Thickness to length is what the
 * eye recognises the sign by, so it scales with the box and is never set by
 * whoever is drawing it. */
export const SIGN_WEIGHT = 2.2;
