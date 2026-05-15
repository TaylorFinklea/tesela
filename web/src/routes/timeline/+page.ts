import { redirect } from "@sveltejs/kit";

// `/timeline` was the legacy chronological daily-notes page. Prism v4
// reaches the same data via the Dashboard's pinned widgets and the
// `timeline` query widget mounted in a pane.
export function load() {
  throw redirect(307, "/v4");
}
