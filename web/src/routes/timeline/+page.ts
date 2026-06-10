import { redirect } from "@sveltejs/kit";

// `/timeline` was the legacy chronological daily-notes page. The Graphite
// shell's journal buffer is the continuous daily view, and query widgets
// cover the rest.
export function load() {
  throw redirect(307, "/g");
}
