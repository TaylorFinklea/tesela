import { redirect } from "@sveltejs/kit";

// Graphite cutover (B2): `/` lands in the Graphite shell — `/g` is the
// default chrome now. The shell's default empty page buffer renders the
// continuous journal, so the user still arrives on their daily note
// without an extra hop. The Prism v4 chrome stays reachable at `/v4`
// until the parity checklist clears its deletion (B3).
export function load() {
  throw redirect(307, "/g");
}
