// Disable SSR — Tesela is a client-side app talking to a local server.
// This also fixes @tabler/icons-svelte which ships .svelte files that
// Node.js can't process during SSR.
export const ssr = false;
