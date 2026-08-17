import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  kit: {
    // Every route is prerendered, so no fallback page is needed.
    adapter: adapter({ fallback: null, strict: true }),
  },
};
