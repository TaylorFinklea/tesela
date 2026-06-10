import { redirect } from "@sveltejs/kit";

// `/daily` is a shortcut to today's note. The Graphite shell's default
// empty page buffer renders the continuous journal, so a plain redirect
// lands the user exactly where the legacy /daily flow used to drop them.
export function load() {
  throw redirect(307, "/g");
}
