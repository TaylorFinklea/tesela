import { redirect } from "@sveltejs/kit";

// `/properties` was the legacy property browser. Properties are reachable
// in v4 via `K` peek with kind = `properties`. The design playground
// pages (`/design/properties`) stay around as developer sandboxes.
export function load() {
  throw redirect(307, "/v4");
}
