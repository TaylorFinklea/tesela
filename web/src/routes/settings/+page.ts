import { redirect } from "@sveltejs/kit";

// `/settings` is just a router; the first tab (`general`) is the
// canonical landing page. SSR is off in `+layout.ts`, so this runs
// in the browser.
export function load() {
  throw redirect(307, "/settings/general");
}
