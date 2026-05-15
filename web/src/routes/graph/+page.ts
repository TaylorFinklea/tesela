import { redirect } from "@sveltejs/kit";

// `/graph` was the legacy fullscreen graph route. v4 owns this as
// either the `g` fullscreen overlay or a pane kind, depending on
// whether the user wants it persistent or transient.
export function load() {
  throw redirect(307, "/v4");
}
