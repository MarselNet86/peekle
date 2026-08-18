import '@testing-library/jest-dom/vitest';

// svelte/motion asks for prefers-reduced-motion the moment it is imported, and
// jsdom ships no matchMedia. Reporting no preference keeps the springs running
// so the component tests see the same behaviour the product has.
if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}
