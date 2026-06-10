import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

// B3: legacy `/v4/p/<slug>` deep links forward into the Graphite shell
// with the slug pre-loaded as the focused buffer (same hash mechanism as
// the canonical /p/<slug> redirect).
export const load: PageLoad = ({ params }) => {
  throw redirect(307, `/g#tile=${encodeURIComponent(params.id)}`);
};
