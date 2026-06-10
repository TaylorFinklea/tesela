import { redirect } from "@sveltejs/kit";

// Graphite cutover (B2): `/` lands in the Graphite shell — `/g` is the
// default chrome now. The shell's default empty page buffer renders the
// continuous journal, so the user still arrives on their daily note
// without an extra hop. (B3 deleted the legacy v4/v5 chromes; `/v4`
// keeps a redirect stub so stale links land here.)
export function load() {
  throw redirect(307, "/g");
}
