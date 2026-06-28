import { expect, test } from "@playwright/test";

/**
 * Phase 2 desktop presence (Taylor, 2026-06-27): a peer's live caret shows as a
 * remote cursor on another client, in real time, over the WS.
 *
 * Two independent browser contexts (→ distinct peer ids) open the SAME page.
 * Page A focuses a block and moves its caret; the caret is published as an
 * ephemeral `PRES` frame, fanned out by the server (`route_inbound_binary`,
 * never touching the engine), and rendered on page B as a `.cm-remote-cursor`
 * CodeMirror decoration. No reload, no relay needed — the WS in-memory fan-out
 * carries it.
 */
const SLUG = process.env.TESELA_E2E_DAILY_SLUG ?? "e2e-delete-refresh";

test("a peer's caret shows as a live remote cursor on another page", async ({ browser }) => {
  const ctxA = await browser.newContext();
  const ctxB = await browser.newContext();
  try {
    const a = await ctxA.newPage();
    const b = await ctxB.newPage();
    await a.goto(`/p/${SLUG}`);
    await b.goto(`/p/${SLUG}`);

    // Both editors render their block(s).
    await expect(a.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });
    await expect(b.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });
    // B has no remote cursor before A does anything.
    await expect(b.locator(".cm-remote-cursor")).toHaveCount(0);

    // A focuses the first block and moves its caret (a real edit guarantees a
    // caret move → a published presence frame).
    await a.locator(".cm-content").first().click();
    await a.keyboard.press("End");
    await a.keyboard.type("A");

    // B renders A's caret as a remote cursor decoration, live.
    await expect(b.locator(".cm-remote-cursor")).toHaveCount(1, { timeout: 12_000 });
  } finally {
    await ctxA.close();
    await ctxB.close();
  }
});
