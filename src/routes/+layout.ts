// The overlay ships as a static bundle inside the app. No server, no hydration
// from a server render.
export const ssr = false;
export const prerender = true;

// Emit `<route>/index.html` so the same URL resolves under `vite dev` and under
// the Tauri asset protocol.
export const trailingSlash = 'always';
