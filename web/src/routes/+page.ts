import { redirect } from "@sveltejs/kit";

// Phase 6 default-route swap. `/` lands in Prism v4 — the redesign now
// owns the daily-driver entry. The v4 page auto-seeds today's daily note
// into the focused pane on first mount, so the user still arrives on
// their journal without an extra redirect hop. Legacy `/p/<id>`,
// `/settings/*`, `/timeline`, `/graph`, `/properties`, `/design` routes
// keep their chrome until they're folded into v4 panes.
export function load() {
  throw redirect(307, "/v4");
}
