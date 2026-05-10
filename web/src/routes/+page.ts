import { redirect } from "@sveltejs/kit";
import { api } from "$lib/api-client";

// `/` defaults to today's Daily note — that's the daily-driver entry point.
// The previous tabular Pages view is still reachable at `/p/pages` (and via
// the leader chord `g p`). SSR is disabled in `+layout.ts` so this `load`
// runs in the browser; the API call hits the local tesela-server.
export async function load() {
  try {
    const daily = await api.getDailyNote();
    throw redirect(307, `/p/${encodeURIComponent(daily.id)}`);
  } catch (err: unknown) {
    // SvelteKit's `redirect()` throws a sentinel — let it propagate.
    if (
      err && typeof err === "object" && "status" in err && "location" in err
    ) {
      throw err;
    }
    // If the daily lookup fails (server down, brand-new mosaic without
    // any daily yet), fall through to /p/pages so the user lands somewhere
    // useful instead of a blank screen.
    throw redirect(307, "/p/pages");
  }
}
