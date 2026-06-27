import { expect, test } from "@playwright/test";

/**
 * Regression: an INBOUND block delete (from a peer — iOS/another device) must
 * be reflected on the live refresh, WITHOUT a manual reload (Taylor,
 * 2026-06-27). Runs against a single page note — the bug lives in the shared
 * `BlockOutliner.applyExternalReparse`, which renders both pages and the
 * journal's per-day sections, so a page exercises the exact fix (and renders
 * headlessly without the journal's workspace-view setup).
 *
 * Root cause that this guards: `BlockOutliner.applyExternalReparse` skipped the
 * reparse when `targetBody === lastSentBody`. After an inbound ADD the rendered
 * blocks diverge from `lastSentBody` (which only advances on a LOCAL save), so
 * a later inbound DELETE that restores exactly `lastSentBody` byte-for-byte was
 * mistaken for our own echo and dropped — the block stayed on screen until a
 * full refresh. The fix compares the CURRENT render
 * (`buildFullContent(blocks).bodyOnly`) instead. This test reproduces the exact
 * order (add, then delete-back-to-baseline) that triggered it.
 *
 * The mutations go through Playwright's `request` context (NOT the page's
 * api-client), so they are NOT own-echo-suppressed — they drive the same
 * `WsEvent::NoteUpdated` → refetch → reconcile path an iOS edit does.
 */
const SLUG = process.env.TESELA_E2E_DAILY_SLUG ?? "2026-06-26";
const BID = "deadbeef-0000-4000-8000-0000000000e2";
const PROBE = "E2E_DELETE_PROBE";

test("inbound block delete is applied on live refresh without a manual reload", async ({ page, request }) => {
  await page.goto(`/p/${SLUG}`);
  // The outliner renders with content.
  await expect(page.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });

  // Inbound ADD → must auto-show (proves the live-refresh path works at all).
  const add = await request.post(`/api/notes/${SLUG}/blocks`, {
    data: { ops: [{ kind: "upsert", bid: BID, text: PROBE, indent_level: 0 }] },
  });
  expect(add.ok()).toBeTruthy();
  await expect(page.getByText(PROBE)).toBeVisible();

  // Inbound DELETE → must auto-REMOVE without a manual refresh (the bug).
  const del = await request.delete(`/api/notes/${SLUG}/blocks/${BID}`);
  expect(del.status()).toBe(204);
  await expect(page.getByText(PROBE)).toHaveCount(0);
});
