import { redirect } from "@sveltejs/kit";

// `/daily` is a shortcut to today's note. v4's `+page.svelte` already
// auto-seeds the daily note into the focused empty editor pane on first
// mount, so a plain redirect lands the user exactly where the legacy
// /daily flow used to drop them.
export function load() {
  throw redirect(307, "/v4");
}
