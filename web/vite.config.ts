import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const apiTarget = process.env.TESELA_API_TARGET ?? 'http://127.0.0.1:7474';
const wsTarget = apiTarget.replace(/^http/, 'ws');

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		host: '0.0.0.0',
		proxy: {
			'/api': {
				target: apiTarget,
				changeOrigin: true,
				rewrite: (p) => p.replace(/^\/api/, ''),
			},
			'/ws': {
				target: wsTarget,
				ws: true,
			},
			// Live dictation session (dictation P2) — same-origin WS like /ws.
			'/transcription/stream': {
				target: wsTarget,
				ws: true,
			},
		},
	},
});
