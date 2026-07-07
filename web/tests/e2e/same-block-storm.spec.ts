import { expect, test, type Page } from "@playwright/test";

/**
 * The 9iy storm shape (tesela-baa): two clients typing rapidly in the SAME
 * block at the same time. Expected: both sides' characters survive and the
 * block converges to one interleaved text — no side destroyed.
 *
 * Pre-baa this failed on any editor whose note wasn't the shell's focused
 * buffer (the doc registry's predecessor was a focus-keyed singleton): both
 * clients fell back to 500ms whole-block writes and the last writer clobbered
 * the other side's burst. Post-baa every mounted BlockOutliner acquires its
 * note's Loro doc, so both clients author per-keystroke splices onto the
 * shared LoroText and the CRDT interleaves them.
 *
 * Assertions count each side's marker CHARACTERS (interleaving may legally
 * split a marker string apart) — survival, not ordering, is the property 9iy
 * violated.
 */
const SLUG = process.env.TESELA_E2E_DAILY_SLUG ?? "e2e-delete-refresh";

const A_CHARS = "xyz";
const B_CHARS = "qwk";
const REPEATS = 4;

function countChar(haystack: string, ch: string): number {
  let n = 0;
  for (const c of haystack) if (c === ch) n++;
  return n;
}

async function firstBlockText(page: Page): Promise<string> {
  return (await page.locator(".cm-content").first().textContent()) ?? "";
}

/** Click into the seed block and enter insert mode at end-of-line. `A` is
 *  vim append-at-EOL when vim is active; with vim off it types a literal 'A',
 *  which the assertions ignore (only marker chars are counted). */
async function enterInsertAtEol(page: Page): Promise<void> {
  await page.locator(".cm-content").first().click();
  await page.keyboard.press("End");
  await page.keyboard.type("A", { delay: 50 });
}

test("same-block concurrent typing interleaves — both sides survive", async ({ browser }) => {
  const ctxA = await browser.newContext();
  const ctxB = await browser.newContext();
  try {
    const a = await ctxA.newPage();
    const b = await ctxB.newPage();
    await a.goto(`/p/${SLUG}`);
    await b.goto(`/p/${SLUG}`);
    await expect(a.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });
    await expect(b.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });

    // Let the per-note Loro docs bootstrap + the editors bind (the block
    // subscription retries for ~3s after mount).
    await a.waitForTimeout(3_500);
    await b.waitForTimeout(3_500);

    await enterInsertAtEol(a);
    await enterInsertAtEol(b);

    // Type simultaneously: interleaved per-character bursts from both sides.
    const typeA = (async () => {
      for (let i = 0; i < REPEATS; i++) {
        for (const ch of A_CHARS) await a.keyboard.type(ch, { delay: 30 });
      }
    })();
    const typeB = (async () => {
      for (let i = 0; i < REPEATS; i++) {
        for (const ch of B_CHARS) await b.keyboard.type(ch, { delay: 30 });
      }
    })();
    await Promise.all([typeA, typeB]);

    // Both sides converge to the same text containing EVERY character each
    // side typed. (The 9iy failure mode: one side's burst destroyed.)
    await expect
      .poll(
        async () => {
          const ta = await firstBlockText(a);
          const tb = await firstBlockText(b);
          const allSurvived = [...A_CHARS, ...B_CHARS].every(
            (ch) => countChar(ta, ch) === REPEATS && countChar(tb, ch) === REPEATS,
          );
          return allSurvived && ta === tb ? "converged" : `a=${JSON.stringify(ta)} b=${JSON.stringify(tb)}`;
        },
        { timeout: 20_000, intervals: [500] },
      )
      .toBe("converged");
  } finally {
    await ctxA.close();
    await ctxB.close();
  }
});
