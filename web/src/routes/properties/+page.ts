import { redirect } from "@sveltejs/kit";

// `/properties` was the legacy property browser. Properties are reachable
// in the Graphite shell via peek (⌘I / leader). The design playground
// pages (`/design/properties`) stay around as developer sandboxes.
export function load() {
  throw redirect(307, "/g");
}
