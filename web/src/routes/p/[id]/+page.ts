import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

// Phase 6 default-route swap. The canonical /p/<slug> URL now routes the
// user into Prism v4 with the slug pre-loaded as the focused pane's
// active tile. The hash carries the slug across the redirect; v4's
// `+page.svelte` consumes it on mount and clears the hash via
// `history.replaceState`, so the URL bar settles at `/v4`.
//
// The legacy single-note view (the long `+page.svelte` next to this
// file) stays in the repo as a snapshot of the chrome we're moving
// away from, but the redirect prevents it from rendering — every
// `/p/<id>` link lands in v4 instead.
export const load: PageLoad = ({ params }) => {
  throw redirect(307, `/v4#tile=${encodeURIComponent(params.id)}`);
};
