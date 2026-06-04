import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
		runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
	},
	kit: {
		// Static SPA build (the app is client-rendered — `ssr = false` in the
		// root +layout, zero server routes). `fallback` makes every unmatched
		// path serve index.html so client-side routing (/g, /p/..) works. This
		// is the artifact the desktop (Tauri) shell's embedded tesela-server
		// serves; the hosted web can use the same build behind any static host.
		adapter: adapter({ fallback: 'index.html' })
	}
};

export default config;
