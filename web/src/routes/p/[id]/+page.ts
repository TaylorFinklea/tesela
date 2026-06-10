import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

// Graphite cutover (B2): the canonical /p/<slug> URL deep-links into the
// Graphite shell with the slug pre-loaded as the focused buffer. The hash
// carries the slug across the redirect; /g's `+page.svelte` consumes it on
// mount and clears the hash via `history.replaceState`, so the URL bar
// settles at `/g` — the same mechanism the v4 chrome used.
export const load: PageLoad = ({ params }) => {
  throw redirect(307, `/g#tile=${encodeURIComponent(params.id)}`);
};
