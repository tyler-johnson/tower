import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // A fallback SPA, not a prerendered page: an open flight is a real
    // path (`/f/pi-2118.94`), and only the client knows which paths
    // exist. Every non-/api request answers index.html and the router
    // takes it from there.
    adapter: adapter({ fallback: "index.html" }),
  },
};

export default config;
