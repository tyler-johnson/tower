import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		proxy: {
			// 127.0.0.1, not localhost: the server binds v4 only, and node
			// may resolve localhost to ::1.
			'/api': { target: 'http://127.0.0.1:7420' }
		}
	}
});
