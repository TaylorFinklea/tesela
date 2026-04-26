import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		host: '0.0.0.0',
		proxy: {
			'/api': {
				target: 'http://127.0.0.1:7474',
				changeOrigin: true,
				rewrite: (p) => p.replace(/^\/api/, ''),
			},
			'/ws': {
				target: 'ws://127.0.0.1:7474',
				ws: true,
			},
		},
	},
});
