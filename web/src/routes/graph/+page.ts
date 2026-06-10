import { redirect } from "@sveltejs/kit";

// `/graph` was the legacy fullscreen graph route. In the Graphite shell
// the graph lives behind the fullscreen overlay (⌘G / leader). Like the
// old /v4 stub, this lands on the shell without auto-opening the overlay.
export function load() {
  throw redirect(307, "/g");
}
