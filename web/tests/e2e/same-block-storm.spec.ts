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
const MARKER_CHARS = [...new Set([...A_CHARS, ...B_CHARS])];

type ClientSample = {
  bid: string | null;
  cm: string;
  loro: string | null;
};

type ServerSample = {
  status: number;
  body: string;
  content: string | null;
};

function countChar(haystack: string, ch: string): number {
  let n = 0;
  for (const c of haystack) if (c === ch) n++;
  return n;
}

async function settleClient(page: Page): Promise<void> {
  await page.evaluate(async (slug) => {
    const registryUrl = "/src/lib/loro/note-doc-registry.svelte.ts";
    const registry = await import(registryUrl);
    await registry.settleNoteDocsAtServer([slug]);
  }, SLUG);
}

async function sampleClient(page: Page): Promise<ClientSample> {
  return page.evaluate(async (slug) => {
    const editor = document.querySelector<HTMLElement>(".cm-content");
    const row = editor?.closest<HTMLElement>("[data-block-bid]") ?? null;
    const clone = editor?.cloneNode(true) as HTMLElement | undefined;
    clone?.querySelectorAll(".cm-remote-cursor").forEach((cursor) => cursor.remove());
    const bid = row?.dataset.blockBid ?? null;
    const registryUrl = "/src/lib/loro/note-doc-registry.svelte.ts";
    const registry = await import(registryUrl);
    return {
      bid,
      cm: clone?.textContent ?? "",
      loro: bid ? registry.getNoteDoc(slug)?.blockTextByBid(bid) ?? null : null,
    };
  }, SLUG);
}

async function sampleServer(page: Page): Promise<ServerSample> {
  return page.evaluate(async (slug) => {
    const response = await fetch(`/api/notes/${encodeURIComponent(slug)}`);
    const body = await response.text();
    let content: string | null = null;
    try {
      const parsed = JSON.parse(body) as { content?: unknown };
      if (typeof parsed.content === "string") content = parsed.content;
    } catch {
      // Preserve the raw body in diagnostics when the response is not JSON.
    }
    return { status: response.status, body, content };
  }, SLUG);
}

function expectedMarkerCounts(baseline: string): Map<string, number> {
  return new Map(
    MARKER_CHARS.map((ch) => [ch, countChar(baseline, ch) + REPEATS]),
  );
}

function allMarkersSurvive(text: string, expectedCounts: Map<string, number>): boolean {
  return [...expectedCounts].every(([ch, expected]) => countChar(text, ch) === expected);
}

function clientsConverged(
  a: ClientSample,
  b: ClientSample,
  expectedCounts: Map<string, number>,
): boolean {
  return a.bid !== null
    && a.bid === b.bid
    && a.loro !== null
    && b.loro !== null
    && a.cm === b.cm
    && a.cm === a.loro
    && b.cm === b.loro
    && allMarkersSurvive(a.cm, expectedCounts);
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

    const [baselineA, baselineB] = await Promise.all([sampleClient(a), sampleClient(b)]);
    expect(baselineA.bid).not.toBeNull();
    expect(baselineA.bid).toBe(baselineB.bid);
    expect(baselineA.cm).toBe(baselineB.cm);
    expect(baselineA.cm).toBe(baselineA.loro);
    expect(baselineB.cm).toBe(baselineB.loro);
    const expectedCounts = expectedMarkerCounts(baselineA.cm);

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

    await Promise.all([settleClient(a), settleClient(b)]);
    await a.waitForTimeout(500);

    // Both sides converge to the same text containing EVERY character each
    // side typed. (The 9iy failure mode: one side's burst destroyed.)
    let liveA = await sampleClient(a);
    let liveB = await sampleClient(b);
    let liveConverged = false;
    try {
      await expect
        .poll(
          async () => {
            [liveA, liveB] = await Promise.all([sampleClient(a), sampleClient(b)]);
            return clientsConverged(liveA, liveB, expectedCounts) ? "converged" : "diverged";
          },
          { timeout: 20_000, intervals: [500] },
        )
        .toBe("converged");
      liveConverged = true;
    } catch {
      // Continue through server and cold-reload sampling before surfacing the failure.
    }

    const server = await sampleServer(a);
    await Promise.all([a.reload(), b.reload()]);
    await expect(a.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });
    await expect(b.locator(".cm-line").first()).toBeVisible({ timeout: 15_000 });
    await a.waitForTimeout(3_500);
    const [reloadedA, reloadedB] = await Promise.all([sampleClient(a), sampleClient(b)]);

    const persisted = liveConverged
      && server.status === 200
      && server.content?.includes(liveA.cm) === true
      && clientsConverged(reloadedA, reloadedB, expectedCounts)
      && reloadedA.cm === liveA.cm;
    if (!persisted) {
      throw new Error(`same-block convergence diagnostics: ${JSON.stringify({
        baseline: { a: baselineA, b: baselineB },
        live: { a: liveA, b: liveB },
        server,
        reloaded: { a: reloadedA, b: reloadedB },
      }, null, 2)}`);
    }
  } finally {
    await ctxA.close();
    await ctxB.close();
  }
});
