import { expect, test, type APIRequestContext, type Locator, type Page } from "@playwright/test";

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing ${name}; run this spec through tests/e2e/run.mjs`);
  return value;
}

const SOURCE = requiredEnv("TESELA_E2E_SOURCE_DAILY");
const DESTINATION = requiredEnv("TESELA_E2E_DEST_DAILY");
const ABSENT = requiredEnv("TESELA_E2E_ABSENT_DAILY");

const BIDS = {
  sameBeforeTarget: "10000000-0000-4000-8000-000000000001",
  sameBeforeRoot: "10000000-0000-4000-8000-000000000002",
  sameInsideTarget: "10000000-0000-4000-8000-000000000003",
  sameInsideTargetChild: "10000000-0000-4000-8000-000000000004",
  sameInsideRoot: "10000000-0000-4000-8000-000000000005",
  sameAfterRoot: "10000000-0000-4000-8000-000000000006",
  sameAfterRootChild: "10000000-0000-4000-8000-000000000007",
  sameAfterTarget: "10000000-0000-4000-8000-000000000008",
  sameAfterTargetChild: "10000000-0000-4000-8000-000000000009",

  crossRoot: "20000000-0000-4000-8000-000000000001",
  crossChild: "20000000-0000-4000-8000-000000000002",
  crossGrandchild: "20000000-0000-4000-8000-000000000003",
  existingAppendRoot: "20000000-0000-4000-8000-000000000004",
  existingAppendChild: "20000000-0000-4000-8000-000000000005",
  absentAppendRoot: "20000000-0000-4000-8000-000000000006",
  invalidRoot: "20000000-0000-4000-8000-000000000007",
  invalidChild: "20000000-0000-4000-8000-000000000008",
  invalidTarget: "20000000-0000-4000-8000-000000000009",
  keyboardCancelRoot: "20000000-0000-4000-8000-000000000010",
  keyboardBeforeRoot: "20000000-0000-4000-8000-000000000011",
  keyboardInsideRoot: "20000000-0000-4000-8000-000000000012",
  keyboardAfterRoot: "20000000-0000-4000-8000-000000000013",
  retryRoot: "20000000-0000-4000-8000-000000000014",
  altParent: "20000000-0000-4000-8000-000000000015",
  altMover: "20000000-0000-4000-8000-000000000016",
  altSibling: "20000000-0000-4000-8000-000000000017",
  racePointerRoot: "20000000-0000-4000-8000-000000000018",
  raceAltRoot: "20000000-0000-4000-8000-000000000019",
  raceAltSibling: "20000000-0000-4000-8000-000000000020",
  crossBeforeRoot: "20000000-0000-4000-8000-000000000021",
  crossBeforeChild: "20000000-0000-4000-8000-000000000022",
  crossAfterRoot: "20000000-0000-4000-8000-000000000023",
  crossAfterChild: "20000000-0000-4000-8000-000000000024",
  untrustedFocusRoot: "20000000-0000-4000-8000-000000000025",
  ambiguousRoot: "20000000-0000-4000-8000-000000000026",
  propertyRaceRoot: "20000000-0000-4000-8000-000000000027",
  propertyFailureRoot: "20000000-0000-4000-8000-000000000028",
  webkitFallbackRoot: "20000000-0000-4000-8000-000000000029",
  webkitFallbackChild: "20000000-0000-4000-8000-000000000030",
  directPointerRoot: "20000000-0000-4000-8000-000000000031",
  directPointerChild: "20000000-0000-4000-8000-000000000032",

  crossTarget: "30000000-0000-4000-8000-000000000001",
  crossTargetChild: "30000000-0000-4000-8000-000000000002",
  existingEnd: "30000000-0000-4000-8000-000000000003",
  keyboardBeforeTarget: "30000000-0000-4000-8000-000000000004",
  keyboardInsideTarget: "30000000-0000-4000-8000-000000000005",
  keyboardInsideTargetChild: "30000000-0000-4000-8000-000000000006",
  keyboardAfterTarget: "30000000-0000-4000-8000-000000000007",
  keyboardAfterTargetChild: "30000000-0000-4000-8000-000000000008",
  retryTarget: "30000000-0000-4000-8000-000000000009",
  racePointerTarget: "30000000-0000-4000-8000-000000000010",
  crossBeforeTarget: "30000000-0000-4000-8000-000000000011",
  crossBeforeTargetChild: "30000000-0000-4000-8000-000000000012",
  crossAfterTarget: "30000000-0000-4000-8000-000000000013",
  crossAfterTargetChild: "30000000-0000-4000-8000-000000000014",
  untrustedFocusTarget: "30000000-0000-4000-8000-000000000015",
  ambiguousTarget: "30000000-0000-4000-8000-000000000016",
  propertyRaceTarget: "30000000-0000-4000-8000-000000000017",
  propertyFailureTarget: "30000000-0000-4000-8000-000000000018",
  webkitFallbackTarget: "30000000-0000-4000-8000-000000000019",
  directPointerTarget: "30000000-0000-4000-8000-000000000020",
} as const;

const MOVE_ROUTE = "**/api/blocks/move-subtree";
const SET_PROPERTY_ROUTE = "**/api/blocks/set-property";
const RECOVERY_STORAGE_KEY = "tesela:block-move-recovery:v1";

function day(page: Page, date: string): Locator {
  return page.locator(`.day[data-daily="${date}"]`);
}

function row(page: Page, bid: string): Locator {
  return page.locator(`[data-block-bid="${bid}"]`);
}

async function mountDay(page: Page, date: string): Promise<Locator> {
  const section = day(page, date);
  await expect(section).toBeAttached({ timeout: 15_000 });
  await section.scrollIntoViewIfNeeded();
  await expect(section.locator("[data-block-outliner]")).toBeVisible({ timeout: 15_000 });
  return section;
}

async function openJournal(page: Page): Promise<{ source: Locator; destination: Locator }> {
  await page.goto("/g");
  const source = await mountDay(page, SOURCE);
  const destination = await mountDay(page, DESTINATION);
  return { source, destination };
}

async function dragToPlacement(
  sourceHandle: Locator,
  targetRow: Locator,
  placement: "before" | "inside" | "after",
): Promise<void> {
  await expect(sourceHandle).toBeVisible();
  await expect(targetRow).toBeVisible();
  const size = await targetRow.evaluate((element) => {
    const rect = element.getBoundingClientRect();
    return { width: rect.width, height: rect.height };
  });
  const y = placement === "before"
    ? 2
    : placement === "inside"
      ? size.height / 2
      : Math.max(2, size.height - 2);
  await sourceHandle.dragTo(targetRow, {
    targetPosition: { x: Math.min(24, Math.max(2, size.width / 2)), y },
  });
}

async function dragAcrossDaysToPlacement(
  page: Page,
  sourceHandle: Locator,
  targetRow: Locator,
  placement: "before" | "inside" | "after",
): Promise<void> {
  await sourceHandle.scrollIntoViewIfNeeded();
  await expect(sourceHandle).toBeVisible();
  const sourceBox = await sourceHandle.boundingBox();
  if (!sourceBox) throw new Error("Drag source has no bounding box");

  const sourcePoint = {
    x: sourceBox.x + sourceBox.width / 2,
    y: sourceBox.y + sourceBox.height / 2,
  };
  await page.mouse.move(sourcePoint.x, sourcePoint.y);
  await page.mouse.down();
  let mouseDown = true;
  try {
    // Cross the browser's native drag threshold before scrolling the distant
    // target into view. Locator.dragTo scrolls first, which can move the
    // pressed source off-screen before Chromium emits dragstart.
    await page.mouse.move(sourcePoint.x + 8, sourcePoint.y + 2, { steps: 4 });
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "selecting");
    await targetRow.scrollIntoViewIfNeeded();
    const targetBox = await targetRow.boundingBox();
    if (!targetBox) throw new Error("Drop target has no bounding box");
    const y = placement === "before"
      ? targetBox.y + 2
      : placement === "inside"
        ? targetBox.y + targetBox.height / 2
        : targetBox.y + Math.max(2, targetBox.height - 2);
    await page.mouse.move(
      targetBox.x + Math.min(24, Math.max(2, targetBox.width / 2)),
      y,
      { steps: 12 },
    );
    await page.mouse.up();
    mouseDown = false;
  } finally {
    if (mouseDown) await page.mouse.up();
  }
}

async function dispatchToPlacement(
  page: Page,
  sourceHandle: Locator,
  target: Locator,
  placement: "before" | "inside" | "after",
): Promise<void> {
  await expect(sourceHandle).toBeAttached();
  await target.scrollIntoViewIfNeeded();
  const box = await target.boundingBox();
  if (!box) throw new Error("Drop target has no bounding box");
  const clientY = placement === "before"
    ? box.y + 2
    : placement === "inside"
      ? box.y + box.height / 2
      : box.y + Math.max(2, box.height - 2);
  const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
  const event = {
    dataTransfer,
    clientX: box.x + Math.min(24, Math.max(2, box.width / 2)),
    clientY,
  };
  await sourceHandle.dispatchEvent("dragstart", { dataTransfer });
  await target.dispatchEvent("dragover", event);
  await expect(target).toHaveAttribute("data-drop-placement", placement);
  await target.dispatchEvent("drop", event);
  await sourceHandle.dispatchEvent("dragend", { dataTransfer });
  await dataTransfer.dispose();
}

async function dispatchToPlacementWithoutCustomMime(
  page: Page,
  sourceHandle: Locator,
  target: Locator,
  placement: "before" | "inside" | "after",
): Promise<void> {
  await target.scrollIntoViewIfNeeded();
  const box = await target.boundingBox();
  if (!box) throw new Error("Drop target has no bounding box");
  const clientY = placement === "before"
    ? box.y + 2
    : placement === "inside"
      ? box.y + box.height / 2
      : box.y + Math.max(2, box.height - 2);
  const dataTransfer = await page.evaluateHandle(() => {
    const transfer = new DataTransfer();
    const setData = transfer.setData.bind(transfer);
    Object.defineProperty(transfer, "setData", {
      value(type: string, value: string) {
        if (type === "application/x-tesela-block-move") {
          throw new DOMException("custom MIME unavailable");
        }
        setData(type, value);
      },
    });
    return transfer;
  });
  const event = {
    dataTransfer,
    clientX: box.x + Math.min(24, Math.max(2, box.width / 2)),
    clientY,
  };
  await sourceHandle.dispatchEvent("dragstart", { dataTransfer });
  await target.dispatchEvent("dragover", event);
  await expect(target).toHaveAttribute("data-drop-placement", placement);
  await target.dispatchEvent("drop", event);
  await sourceHandle.dispatchEvent("dragend", { dataTransfer });
  await dataTransfer.dispose();
}

async function dispatchAppend(
  page: Page,
  sourceHandle: Locator,
  targetDay: Locator,
): Promise<void> {
  const header = targetDay.locator("[data-move-day-target='true']");
  await header.scrollIntoViewIfNeeded();
  const box = await header.boundingBox();
  if (!box) throw new Error("Day header has no bounding box");
  const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
  const event = {
    dataTransfer,
    clientX: box.x + box.width / 2,
    clientY: box.y + box.height / 2,
  };
  await sourceHandle.dispatchEvent("dragstart", { dataTransfer });
  await header.dispatchEvent("dragover", event);
  await expect(targetDay).toHaveAttribute("data-drop-placement", "append");
  await header.dispatchEvent("drop", event);
  await sourceHandle.dispatchEvent("dragend", { dataTransfer });
  await dataTransfer.dispose();
}

async function waitForMoveIdle(page: Page): Promise<void> {
  await expect.poll(() => page.locator(".journal").getAttribute("data-move-mode"), {
    timeout: 20_000,
  }).toBeNull();
}

async function bidOrder(section: Locator): Promise<string[]> {
  return section.locator("[data-block-bid]").evaluateAll((elements) =>
    elements.map((element) => element.getAttribute("data-block-bid") ?? ""),
  );
}

async function expectOrdered(section: Locator, bids: string[]): Promise<void> {
  await expect.poll(async () => {
    const order = await bidOrder(section);
    const positions = bids.map((bid) => order.indexOf(bid));
    return positions.every((position) => position >= 0)
      && positions.every((position, index) => index === 0 || positions[index - 1] < position)
      ? "ordered"
      : JSON.stringify(positions);
  }).toBe("ordered");
}

async function noteContent(request: APIRequestContext, noteId: string): Promise<string> {
  const response = await request.get(`/api/notes/${noteId}`);
  expect(response.ok()).toBeTruthy();
  const note = await response.json() as { content: string };
  return note.content;
}

async function setPropertyThroughApp(
  page: Page,
  blockId: string,
  key: string,
  value: string,
): Promise<{ ok: boolean; message: string }> {
  return page.evaluate(async ({ blockId, key, value }) => {
    const apiClientUrl = "/src/lib/api-client.ts";
    const { api } = await import(apiClientUrl);
    try {
      await api.setBlockProperty(blockId, key, value);
      return { ok: true, message: "" };
    } catch (error) {
      return {
        ok: false,
        message: error instanceof Error ? error.message : String(error),
      };
    }
  }, { blockId, key, value });
}

async function propertyReservationHeld(page: Page, noteId: string): Promise<boolean> {
  return page.evaluate(async (id) => {
    const mutationBarrierUrl = "/src/lib/block-ops-saver.ts";
    const { propertyMutationBarrier } = await import(mutationBarrierUrl);
    return propertyMutationBarrier.isReserved(id);
  }, noteId);
}

function occurrences(haystack: string, needle: string): number {
  return haystack.split(needle).length - 1;
}

async function focusNormal(page: Page, bid: string): Promise<void> {
  const content = row(page, bid).locator(".cm-content");
  await content.click();
  await expect(content).toBeFocused();
  await page.keyboard.press("Escape");
}

async function startLeaderMove(page: Page, bid: string): Promise<void> {
  await focusNormal(page, bid);
  await page.keyboard.press("Space");
  await expect(page.locator(".gr-leader")).toBeVisible();
  await page.keyboard.press("a");
  await expect(page.locator(".gr-chord").filter({ hasText: "Move block subtree" })).toBeVisible();
  await page.keyboard.press("m");
  await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "selecting");
  const moveSourceMatches = await row(page, bid).evaluateAll((elements) =>
    elements.some((element) => element.getAttribute("data-move-source") === "true"),
  );
  if (!moveSourceMatches) {
    const diagnostics = await page.evaluate((requestedBid) => {
      const bidsFor = (selector: string) => Array.from(document.querySelectorAll(selector), (element) =>
        element.getAttribute("data-block-bid"),
      );
      const requested = Array.from(document.querySelectorAll(`[data-block-bid="${requestedBid}"]`));
      return {
        requestedBid,
        activeBid: document.activeElement?.closest("[data-block-bid]")?.getAttribute("data-block-bid") ?? null,
        activeClass: document.activeElement?.className ?? null,
        sourceBids: bidsFor('[data-move-source="true"]'),
        keyTargetBids: bidsFor("[data-move-key-target]"),
        requestedRows: requested.map((element) => ({
          noteId: element.getAttribute("data-note-id"),
          text: element.querySelector(".cm-content")?.textContent,
        })),
      };
    }, bid);
    throw new Error(`Move source did not match focused row: ${JSON.stringify(diagnostics)}`);
  }
}

async function startPaletteMove(page: Page, bid: string): Promise<void> {
  await focusNormal(page, bid);
  await page.keyboard.press("Meta+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  await palette.getByRole("combobox").fill("Move block subtree");
  await expect(palette.getByRole("option").first()).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "selecting");
  try {
    await expect(row(page, bid)).toHaveAttribute("data-move-source", "true");
  } catch (error) {
    const snapshot = await paletteMoveSnapshot(page, "source-mismatch", bid);
    const assertion = error instanceof Error ? error.message : String(error);
    throw new Error(
      `Palette move source mismatch: ${JSON.stringify(snapshot)}; assertion: ${assertion}`,
    );
  }
}

type PaletteMoveSnapshot = {
  stage: string;
  requestedBid: string;
  focusedBlock: { id: string; bid: string | null; noteId: string; text: string } | null;
  editorFocused: boolean;
  activeBid: string | null;
  activeClass: string | null;
  sourceBids: string[];
};

async function paletteMoveSnapshot(
  page: Page,
  stage: string,
  requestedBid: string,
): Promise<PaletteMoveSnapshot> {
  return page.evaluate(async ({ stage, requestedBid }) => {
    const currentBlockUrl = "/src/lib/stores/current-block.svelte.ts";
    const focusedEditorUrl = "/src/lib/stores/focused-editor.svelte.ts";
    const [currentBlock, focusedEditor] = await Promise.all([
      import(currentBlockUrl),
      import(focusedEditorUrl),
    ]);
    const block = currentBlock.getFocusedBlock();
    const active = document.activeElement as HTMLElement | null;
    return {
      stage,
      requestedBid,
      focusedBlock: block
        ? {
            id: block.id,
            bid: block.bid ?? null,
            noteId: block.note_id,
            text: block.raw_text,
          }
        : null,
      editorFocused: focusedEditor.isEditorFocused(),
      activeBid: active?.closest("[data-block-bid]")?.getAttribute("data-block-bid") ?? null,
      activeClass: active?.className ?? null,
      sourceBids: [...document.querySelectorAll<HTMLElement>('[data-move-source="true"]')]
        .map((element) => element.dataset.blockBid)
        .filter((value): value is string => value !== undefined),
    };
  }, { stage, requestedBid });
}

async function navigateMoveTo(page: Page, bid: string): Promise<void> {
  const target = row(page, bid);
  for (let presses = 0; presses < 80; presses++) {
    if (await target.getAttribute("data-drop-placement")) return;
    await page.keyboard.press("j");
  }
  throw new Error(`Move mode did not reach target ${bid}`);
}

async function navigateMoveToDay(page: Page, date: string): Promise<void> {
  const target = day(page, date);
  for (let presses = 0; presses < 80; presses++) {
    if (await target.getAttribute("data-drop-placement") === "append") return;
    await page.keyboard.press("j");
  }
  throw new Error(`Move mode did not reach day ${date}`);
}

async function keyboardMove(
  page: Page,
  sourceBid: string,
  targetBid: string,
  placementKey: "b" | "i" | "a",
  exerciseBacktrack = false,
  afterIdle?: () => Promise<void>,
): Promise<void> {
  await startLeaderMove(page, sourceBid);
  await navigateMoveTo(page, targetBid);
  if (exerciseBacktrack) {
    await page.keyboard.press("k");
    await page.keyboard.press("j");
    await expect(row(page, targetBid)).toHaveAttribute("data-drop-placement", "after");
  }
  await page.keyboard.press(placementKey);
  await waitForMoveIdle(page);
  await afterIdle?.();
  await expect(row(page, sourceBid).locator(".cm-content")).toBeFocused();
}

type MovedRowSnapshot = {
  noteId: string | null;
  day: string | null;
  text: string;
  active: boolean;
};

type MovedRowsAtTime = {
  label: "immediate" | "250ms" | "1500ms";
  elapsedMs: number;
  rows: MovedRowSnapshot[];
};

async function movedRowsSnapshot(
  page: Page,
  bid: string,
  label: MovedRowsAtTime["label"],
  startedAt: number,
): Promise<MovedRowsAtTime> {
  const rows = await page.locator(`[data-block-bid="${bid}"]`).evaluateAll((elements) =>
    elements.map((element) => {
      const row = element as HTMLElement;
      const editor = row.querySelector<HTMLElement>(".cm-content");
      const clone = editor?.cloneNode(true) as HTMLElement | undefined;
      clone?.querySelectorAll(".cm-remote-cursor").forEach((cursor) => cursor.remove());
      return {
        noteId: row.dataset.noteId ?? null,
        day: row.closest<HTMLElement>(".day[data-daily]")?.dataset.daily ?? null,
        text: clone?.textContent ?? "",
        active: row.contains(document.activeElement),
      };
    })
  );
  return { label, elapsedMs: Date.now() - startedAt, rows };
}

async function apiBlockSnapshot(
  request: APIRequestContext,
  noteId: string,
  bid: string,
): Promise<{ status: number; occurrences: number; matchingLines: string[] }> {
  const response = await request.get(`/api/notes/${encodeURIComponent(noteId)}`);
  const body = await response.text();
  let content: string | null = null;
  try {
    const parsed = JSON.parse(body) as { content?: unknown };
    if (typeof parsed.content === "string") content = parsed.content;
  } catch {
    // Preserve an empty match list alongside the response status for diagnostics.
  }
  const occurrences = content?.split(bid).length ?? 1;
  const matchingLines = content?.split("\n").filter((line) => line.includes(bid)) ?? [];
  return { status: response.status(), occurrences: occurrences - 1, matchingLines };
}

async function assertCrossDayProjection(
  page: Page,
  request: APIRequestContext,
  bid: string,
): Promise<void> {
  const startedAt = Date.now();
  const timeline = [await movedRowsSnapshot(page, bid, "immediate", startedAt)];
  const immediateRows = timeline[0]?.rows ?? [];
  const immediateSourceCount = immediateRows.filter((entry) => entry.day === SOURCE).length;
  const immediateDestinationRows = immediateRows.filter((entry) => entry.day === DESTINATION);
  const immediateSucceeded = immediateSourceCount === 0
    && immediateDestinationRows.length === 1
    && immediateRows.length === 1
    && immediateDestinationRows[0]?.active === true;
  if (immediateSucceeded) return;

  await page.waitForTimeout(250);
  timeline.push(await movedRowsSnapshot(page, bid, "250ms", startedAt));
  await page.waitForTimeout(1_250);
  timeline.push(await movedRowsSnapshot(page, bid, "1500ms", startedAt));

  const [sourceApi, destinationApi, loro] = await Promise.all([
    apiBlockSnapshot(request, SOURCE, bid),
    apiBlockSnapshot(request, DESTINATION, bid),
    page.evaluate(async ({ source, destination, bid }) => {
      const registryUrl = "/src/lib/loro/note-doc-registry.svelte.ts";
      const registry = await import(registryUrl);
      const sourceDoc = registry.getNoteDoc(source);
      const destinationDoc = registry.getNoteDoc(destination);
      return {
        source: {
          mounted: Boolean(sourceDoc),
          text: sourceDoc?.blockTextByBid(bid) ?? null,
        },
        destination: {
          mounted: Boolean(destinationDoc),
          text: destinationDoc?.blockTextByBid(bid) ?? null,
        },
      };
    }, { source: SOURCE, destination: DESTINATION, bid }),
  ]);
  const diagnostic = {
    bid,
    source: SOURCE,
    destination: DESTINATION,
    immediate: {
      sourceCount: immediateSourceCount,
      destinationCount: immediateDestinationRows.length,
      globalCount: immediateRows.length,
      destinationFocused: immediateDestinationRows[0]?.active === true,
    },
    timeline,
    api: { source: sourceApi, destination: destinationApi },
    loro,
  };
  throw new Error(`Cross-day projection mismatch: ${JSON.stringify(diagnostic)}`);
}

async function dispatchInternalDrop(
  page: Page,
  sourceHandle: Locator,
  target: Locator,
): Promise<void> {
  const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
  const box = await target.boundingBox();
  if (!box) throw new Error("Drop target has no bounding box");
  const event = {
    dataTransfer,
    clientX: box.x + box.width / 2,
    clientY: box.y + box.height / 2,
  };
  await sourceHandle.dispatchEvent("dragstart", { dataTransfer });
  await target.dispatchEvent("dragover", event);
  await target.dispatchEvent("drop", event);
  await sourceHandle.dispatchEvent("dragend", { dataTransfer });
  await dataTransfer.dispose();
}

test.describe("block subtree relocation", () => {
  test.describe.configure({ mode: "serial" });

  test("same-day pointer moves honor before, inside, and after subtree placement", async ({ page }) => {
    const { source } = await openJournal(page);

    await dragToPlacement(
      row(page, BIDS.sameBeforeRoot).locator("[data-move-handle]"),
      row(page, BIDS.sameBeforeTarget),
      "before",
    );
    await waitForMoveIdle(page);
    await expectOrdered(source, [BIDS.sameBeforeRoot, BIDS.sameBeforeTarget]);

    await dragToPlacement(
      row(page, BIDS.sameInsideRoot).locator("[data-move-handle]"),
      row(page, BIDS.sameInsideTarget),
      "inside",
    );
    await waitForMoveIdle(page);
    await expectOrdered(source, [
      BIDS.sameInsideTarget,
      BIDS.sameInsideTargetChild,
      BIDS.sameInsideRoot,
    ]);
    await expect(row(page, BIDS.sameInsideRoot)).toHaveAttribute("style", /padding-left:\s*24px/);

    await dragToPlacement(
      row(page, BIDS.sameAfterRoot).locator("[data-move-handle]"),
      row(page, BIDS.sameAfterTarget),
      "after",
    );
    await waitForMoveIdle(page);
    await expectOrdered(source, [
      BIDS.sameAfterTarget,
      BIDS.sameAfterTargetChild,
      BIDS.sameAfterRoot,
      BIDS.sameAfterRootChild,
    ]);
  });

  test("pointer relocation survives WKWebView rejecting the custom drag MIME", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);

    await dispatchToPlacementWithoutCustomMime(
      page,
      row(page, BIDS.webkitFallbackRoot).locator("[data-move-handle]"),
      row(page, BIDS.webkitFallbackTarget),
      "inside",
    );
    await waitForMoveIdle(page);

    await expect(source.locator(`[data-block-bid="${BIDS.webkitFallbackRoot}"]`)).toHaveCount(0);
    await expectOrdered(destination, [
      BIDS.webkitFallbackTarget,
      BIDS.webkitFallbackRoot,
      BIDS.webkitFallbackChild,
    ]);
    expect(occurrences(await noteContent(request, DESTINATION), BIDS.webkitFallbackChild)).toBe(1);
  });

  test("pointer relocation does not depend on the native HTML drag lifecycle", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);
    const handle = row(page, BIDS.directPointerRoot).locator("[data-move-handle]");
    let moveRequests = 0;
    page.on("request", (outgoing) => {
      if (outgoing.method() === "POST" && outgoing.url().includes("/api/blocks/move-subtree")) {
        moveRequests += 1;
      }
    });
    await handle.evaluate((element) => element.setAttribute("draggable", "false"));

    await dragAcrossDaysToPlacement(
      page,
      handle,
      row(page, BIDS.directPointerChild),
      "inside",
    );
    await waitForMoveIdle(page);
    expect(moveRequests).toBe(0);
    await expect(source.locator(`[data-block-bid="${BIDS.directPointerRoot}"]`)).toHaveCount(1);
    await expect(source.locator(`[data-block-bid="${BIDS.directPointerChild}"]`)).toHaveCount(1);

    await dragAcrossDaysToPlacement(
      page,
      handle,
      row(page, BIDS.directPointerTarget),
      "inside",
    );
    await waitForMoveIdle(page);

    expect(moveRequests).toBe(1);
    await expect(source.locator(`[data-block-bid="${BIDS.directPointerRoot}"]`)).toHaveCount(0);
    await expectOrdered(destination, [
      BIDS.directPointerTarget,
      BIDS.directPointerRoot,
      BIDS.directPointerChild,
    ]);
    expect(occurrences(await noteContent(request, DESTINATION), BIDS.directPointerChild)).toBe(1);
  });

  test("cross-day pointer moves honor before, inside, and after with exact hierarchy", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);
    let moveRequests = 0;
    page.on("request", (outbound) => {
      if (outbound.url().includes("/api/blocks/move-subtree")) moveRequests++;
    });
    const expectMoveRequestCount = async (expected: number) => {
      if (moveRequests === expected) return;
      throw new Error(`Cross-day pointer move issued ${moveRequests}/${expected} requests`);
    };

    await dragAcrossDaysToPlacement(
      page,
      row(page, BIDS.crossBeforeRoot).locator("[data-move-handle]"),
      row(page, BIDS.crossBeforeTarget),
      "before",
    );
    await waitForMoveIdle(page);
    await expectMoveRequestCount(1);
    await expect(row(page, BIDS.crossBeforeRoot).locator(".cm-content")).toBeFocused();

    await dragAcrossDaysToPlacement(
      page,
      row(page, BIDS.crossRoot).locator("[data-move-handle]"),
      row(page, BIDS.crossTarget),
      "inside",
    );
    await waitForMoveIdle(page);
    await expectMoveRequestCount(2);
    await expect(row(page, BIDS.crossRoot).locator(".cm-content")).toBeFocused();

    await dragAcrossDaysToPlacement(
      page,
      row(page, BIDS.crossAfterRoot).locator("[data-move-handle]"),
      row(page, BIDS.crossAfterTarget),
      "after",
    );
    await waitForMoveIdle(page);
    await expectMoveRequestCount(3);
    await expect(row(page, BIDS.crossAfterRoot).locator(".cm-content")).toBeFocused();

    await expect(source.locator(`[data-block-bid="${BIDS.crossBeforeRoot}"]`)).toHaveCount(0);
    await expect(source.locator(`[data-block-bid="${BIDS.crossRoot}"]`)).toHaveCount(0);
    await expect(source.locator(`[data-block-bid="${BIDS.crossAfterRoot}"]`)).toHaveCount(0);
    await expectOrdered(destination, [
      BIDS.crossBeforeRoot,
      BIDS.crossBeforeChild,
      BIDS.crossBeforeTarget,
      BIDS.crossBeforeTargetChild,
      BIDS.crossTarget,
      BIDS.crossTargetChild,
      BIDS.crossRoot,
      BIDS.crossChild,
      BIDS.crossGrandchild,
      BIDS.crossAfterTarget,
      BIDS.crossAfterTargetChild,
      BIDS.crossAfterRoot,
      BIDS.crossAfterChild,
    ]);
    await expect(row(page, BIDS.crossBeforeRoot)).toHaveAttribute("style", /padding-left:\s*0px/);
    await expect(row(page, BIDS.crossBeforeChild)).toHaveAttribute("style", /padding-left:\s*24px/);
    await expect(row(page, BIDS.crossRoot)).toHaveAttribute("style", /padding-left:\s*24px/);
    await expect(row(page, BIDS.crossChild)).toHaveAttribute("style", /padding-left:\s*48px/);
    await expect(row(page, BIDS.crossGrandchild)).toHaveAttribute("style", /padding-left:\s*72px/);
    await expect(row(page, BIDS.crossAfterRoot)).toHaveAttribute("style", /padding-left:\s*0px/);
    await expect(row(page, BIDS.crossAfterChild)).toHaveAttribute("style", /padding-left:\s*24px/);

    let destinationContent = await noteContent(request, DESTINATION);
    expect(occurrences(destinationContent, BIDS.crossBeforeRoot)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossBeforeChild)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossRoot)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossChild)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossGrandchild)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossAfterRoot)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossAfterChild)).toBe(1);
    expect(destinationContent).toContain("status:: doing");

    await page.reload();
    await mountDay(page, SOURCE);
    await mountDay(page, DESTINATION);
    await expect(row(page, BIDS.crossBeforeRoot)).toBeVisible();
    await expect(row(page, BIDS.crossRoot)).toBeVisible();
    await expect(row(page, BIDS.crossAfterRoot)).toBeVisible();
    await expect(day(page, SOURCE).locator(`[data-block-bid="${BIDS.crossRoot}"]`)).toHaveCount(0);
    destinationContent = await noteContent(request, DESTINATION);
    expect(occurrences(destinationContent, BIDS.crossBeforeRoot)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossRoot)).toBe(1);
    expect(occurrences(destinationContent, BIDS.crossAfterRoot)).toBe(1);
  });

  test("existing and untouched synthetic date headers append without a phantom blank", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);

    await dispatchAppend(
      page,
      row(page, BIDS.existingAppendRoot).locator("[data-move-handle]"),
      destination,
    );
    await waitForMoveIdle(page);
    await expectOrdered(destination, [BIDS.existingEnd, BIDS.existingAppendRoot, BIDS.existingAppendChild]);

    const absentBefore = await request.get(`/api/notes/${ABSENT}`);
    expect(absentBefore.status()).toBe(404);
    const absent = await mountDay(page, ABSENT);
    const absentAfterMount = await request.get(`/api/notes/${ABSENT}`);
    expect(absentAfterMount.status()).toBe(404);

    const rejectedTransfer = await page.evaluateHandle(() => {
      const transfer = new DataTransfer();
      transfer.setData("text/plain", "external text drop");
      return transfer;
    });
    await absent.locator("[data-move-day-target='true']").dispatchEvent("drop", {
      dataTransfer: rejectedTransfer,
    });
    await rejectedTransfer.dispose();
    const absentAfterRejectedDrop = await request.get(`/api/notes/${ABSENT}`);
    expect(absentAfterRejectedDrop.status()).toBe(404);

    await dispatchAppend(
      page,
      row(page, BIDS.absentAppendRoot).locator("[data-move-handle]"),
      absent,
    );
    await waitForMoveIdle(page);
    const persistedAbsent = await noteContent(request, ABSENT);
    expect(occurrences(persistedAbsent, BIDS.absentAppendRoot)).toBe(1);
    expect(occurrences(await noteContent(request, SOURCE), BIDS.absentAppendRoot)).toBe(0);
    await expect(absent.locator(`[data-block-bid="${BIDS.absentAppendRoot}"]`)).toBeVisible();
    await expect(source.locator(`[data-block-bid="${BIDS.absentAppendRoot}"]`)).toHaveCount(0);

    await page.reload();
    await mountDay(page, ABSENT);
    await expect(row(page, BIDS.absentAppendRoot)).toBeVisible();
    const absentContent = await noteContent(request, ABSENT);
    expect(occurrences(absentContent, BIDS.absentAppendRoot)).toBe(1);
    expect(absentContent).not.toMatch(/^\s*-\s*(?:<!--\s*bid:[^>]+-->)?\s*$/m);
  });

  test("self, descendant, malformed, and external drops issue zero move requests", async ({ page }) => {
    await openJournal(page);
    let requests = 0;
    await page.route(MOVE_ROUTE, async (route) => {
      requests++;
      await route.continue();
    });

    const handle = row(page, BIDS.invalidRoot).locator("[data-move-handle]");
    await dispatchInternalDrop(page, handle, row(page, BIDS.invalidRoot));
    await dispatchInternalDrop(page, handle, row(page, BIDS.invalidChild));

    const external = await page.evaluateHandle(() => {
      const transfer = new DataTransfer();
      transfer.setData("text/plain", "external text drop");
      return transfer;
    });
    await row(page, BIDS.invalidTarget).locator(".cm-content").dispatchEvent("drop", {
      dataTransfer: external,
    });
    await external.dispose();

    const malformed = await page.evaluateHandle(() => {
      const transfer = new DataTransfer();
      transfer.setData("application/x-tesela-block-move", "not-json");
      return transfer;
    });
    await row(page, BIDS.invalidTarget).dispatchEvent("drop", { dataTransfer: malformed });
    await malformed.dispose();

    await expect.poll(() => requests).toBe(0);
    await expect.poll(() => page.locator(".journal").getAttribute("data-move-mode")).toBeNull();
    await page.unroute(MOVE_ROUTE);
  });

  test("untrusted synthetic input cannot cancel pending move focus restoration", async ({ page }) => {
    const { source, destination } = await openJournal(page);
    await focusNormal(page, BIDS.untrustedFocusRoot);

    let releaseResponse!: () => void;
    let markRequestStarted!: () => void;
    const responseGate = new Promise<void>((resolve) => { releaseResponse = resolve; });
    const requestStarted = new Promise<void>((resolve) => { markRequestStarted = resolve; });
    await page.route(MOVE_ROUTE, async (route) => {
      markRequestStarted();
      await responseGate;
      await route.continue();
    });

    await dispatchToPlacement(
      page,
      row(page, BIDS.untrustedFocusRoot).locator("[data-move-handle]"),
      row(page, BIDS.untrustedFocusTarget),
      "after",
    );
    await requestStarted;
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "pending");

    const isTrusted = await page.evaluate(() => {
      const event = new KeyboardEvent("keydown", {
        key: "i",
        code: "KeyI",
        bubbles: true,
        cancelable: true,
      });
      document.dispatchEvent(event);
      return event.isTrusted;
    });
    expect(isTrusted).toBe(false);

    releaseResponse();
    await waitForMoveIdle(page);
    await expect(source.locator(`[data-block-bid="${BIDS.untrustedFocusRoot}"]`)).toHaveCount(0);
    await expect(destination.locator(`[data-block-bid="${BIDS.untrustedFocusRoot}"]`)).toBeVisible();
    await expect(row(page, BIDS.untrustedFocusRoot).locator(".cm-content")).toBeFocused();
    await page.unroute(MOVE_ROUTE);
  });

  test("palette Escape cancels and leader j/k plus b/i/a move across days", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);
    let canceledRequests = 0;
    await page.route(MOVE_ROUTE, async (route) => {
      canceledRequests++;
      await route.continue();
    });
    await startPaletteMove(page, BIDS.keyboardCancelRoot);
    await page.keyboard.press("Escape");
    await expect.poll(() => page.locator(".journal").getAttribute("data-move-mode")).toBeNull();
    expect(canceledRequests).toBe(0);
    await page.unroute(MOVE_ROUTE);

    await startLeaderMove(page, BIDS.keyboardCancelRoot);
    await navigateMoveToDay(page, DESTINATION);
    await page.keyboard.press("a");
    await waitForMoveIdle(page);
    await assertCrossDayProjection(page, request, BIDS.keyboardCancelRoot);
    await expect(source.locator(`[data-block-bid="${BIDS.keyboardCancelRoot}"]`)).toHaveCount(0);
    await expect(destination.locator(`[data-block-bid="${BIDS.keyboardCancelRoot}"]`)).toBeVisible();
    await expectOrdered(destination, [BIDS.racePointerTarget, BIDS.keyboardCancelRoot]);

    await keyboardMove(
      page,
      BIDS.keyboardBeforeRoot,
      BIDS.keyboardBeforeTarget,
      "b",
      true,
      () => assertCrossDayProjection(page, request, BIDS.keyboardBeforeRoot),
    );
    await expectOrdered(day(page, DESTINATION), [BIDS.keyboardBeforeRoot, BIDS.keyboardBeforeTarget]);

    await keyboardMove(page, BIDS.keyboardInsideRoot, BIDS.keyboardInsideTarget, "i");
    await expectOrdered(day(page, DESTINATION), [
      BIDS.keyboardInsideTarget,
      BIDS.keyboardInsideTargetChild,
      BIDS.keyboardInsideRoot,
    ]);

    await keyboardMove(page, BIDS.keyboardAfterRoot, BIDS.keyboardAfterTarget, "a");
    await expectOrdered(day(page, DESTINATION), [
      BIDS.keyboardAfterTarget,
      BIDS.keyboardAfterTargetChild,
      BIDS.keyboardAfterRoot,
    ]);
  });

  test("Alt Up, Down, and Right persist after reload", async ({ page, request }) => {
    await openJournal(page);

    await focusNormal(page, BIDS.altMover);
    await page.keyboard.press("Alt+ArrowDown");
    await waitForMoveIdle(page);
    await page.reload();
    const source = await mountDay(page, SOURCE);
    await expectOrdered(source, [BIDS.altParent, BIDS.altSibling, BIDS.altMover]);

    await focusNormal(page, BIDS.altMover);
    await page.keyboard.press("Alt+ArrowUp");
    await waitForMoveIdle(page);
    await page.reload();
    await mountDay(page, SOURCE);
    await expectOrdered(day(page, SOURCE), [BIDS.altParent, BIDS.altMover, BIDS.altSibling]);

    await focusNormal(page, BIDS.altMover);
    await page.keyboard.press("Alt+ArrowRight");
    await waitForMoveIdle(page);
    await page.reload();
    await mountDay(page, SOURCE);
    await expectOrdered(day(page, SOURCE), [BIDS.altParent, BIDS.altMover, BIDS.altSibling]);
    const content = await noteContent(request, SOURCE);
    expect(content).toMatch(new RegExp(`^  - ALT_MOVER .*${BIDS.altMover}`, "m"));
  });

  test("foreign and exact retry-safe 503s retain one persisted request across reload", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);
    const sourceBefore = await noteContent(request, SOURCE);
    const destinationBefore = await noteContent(request, DESTINATION);
    let releaseFirst!: () => void;
    const firstGate = new Promise<void>((resolve) => { releaseFirst = resolve; });
    let attempts = 0;
    let firstBody = "";
    let secondBody = "";
    let thirdBody = "";
    const blockingMoveId = "44444444-4444-4444-8444-444444444444";

    await page.route(MOVE_ROUTE, async (route) => {
      attempts++;
      const body = route.request().postData() ?? "";
      if (attempts === 1) {
        firstBody = body;
        await firstGate;
        await route.fulfill({
          status: 503,
          contentType: "application/json",
          body: JSON.stringify({
            error: "Injected earlier recovery gate",
            move_id: blockingMoveId,
            retry_safe: true,
          }),
        });
        return;
      }
      if (attempts === 2) {
        secondBody = body;
        const moveId = (JSON.parse(body) as { move_id: string }).move_id;
        await route.fulfill({
          status: 503,
          contentType: "application/json",
          body: JSON.stringify({ error: "Injected exact recovery gate", move_id: moveId, retry_safe: true }),
        });
        return;
      }
      thirdBody = body;
      await route.continue();
    });

    await dispatchToPlacement(
      page,
      row(page, BIDS.retryRoot).locator("[data-move-handle]"),
      row(page, BIDS.retryTarget),
      "inside",
    );
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "pending");
    await expect(source.locator("[data-block-outliner]")).toHaveAttribute("inert", "");
    await expect(destination.locator("[data-block-outliner]")).toHaveAttribute("inert", "");
    await expect.poll(() => attempts).toBe(1);
    const pendingMarker = await page.evaluate(
      (key) => sessionStorage.getItem(key),
      RECOVERY_STORAGE_KEY,
    );
    expect(pendingMarker).not.toBeNull();
    expect((JSON.parse(pendingMarker!).request as unknown)).toEqual(JSON.parse(firstBody));
    await expect(source.locator(`[data-block-bid="${BIDS.retryRoot}"]`)).toBeVisible();
    await expect(destination.locator(`[data-block-bid="${BIDS.retryRoot}"]`)).toHaveCount(0);

    releaseFirst();
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "retryable");
    await expect(page.locator("[data-move-status='retryable']")).toContainText(/R or Enter/i);
    await expect(page.locator(".tesela-toast-warn")).toContainText(/recovering earlier move/i);
    const expectRetryFrozen = async () => {
      await expect(source.locator("[data-block-outliner]")).toHaveAttribute("inert", "");
      await expect(destination.locator("[data-block-outliner]")).toHaveAttribute("inert", "");
      await expect(source.locator(`[data-block-bid="${BIDS.retryRoot}"]`)).toBeVisible();
      await expect(destination.locator(`[data-block-bid="${BIDS.retryRoot}"]`)).toHaveCount(0);
      expect(await noteContent(request, SOURCE)).toBe(sourceBefore);
      expect(await noteContent(request, DESTINATION)).toBe(destinationBefore);
    };
    await expectRetryFrozen();
    await page.keyboard.press("Escape");
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "retryable");
    await expectRetryFrozen();

    await page.reload();
    await mountDay(page, SOURCE);
    await mountDay(page, DESTINATION);
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "retryable");
    await expectRetryFrozen();

    await page.keyboard.press("r");
    await expect.poll(() => attempts).toBe(2);
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "retryable");
    expect(secondBody).toBe(firstBody);

    await page.keyboard.press("Enter");
    await expect.poll(() => attempts).toBe(3);
    await waitForMoveIdle(page);
    expect(thirdBody).toBe(firstBody);
    expect(await page.evaluate((key) => sessionStorage.getItem(key), RECOVERY_STORAGE_KEY)).toBeNull();
    await expect(source.locator(`[data-block-bid="${BIDS.retryRoot}"]`)).toHaveCount(0);
    await expect(destination.locator(`[data-block-bid="${BIDS.retryRoot}"]`)).toBeVisible();
    await page.unroute(MOVE_ROUTE);
  });

  test("a committed move with a lost response retries the exact request to terminal success", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);
    let attempts = 0;
    let firstBody = "";
    let secondBody = "";
    let markCommitted!: () => void;
    const committed = new Promise<void>((resolve) => { markCommitted = resolve; });

    await page.route(MOVE_ROUTE, async (route) => {
      attempts++;
      const body = route.request().postData() ?? "";
      if (attempts === 1) {
        firstBody = body;
        const upstream = await route.fetch();
        expect(upstream.ok()).toBeTruthy();
        markCommitted();
        await route.abort("failed");
        return;
      }
      secondBody = body;
      await route.continue();
    });

    await dispatchToPlacement(
      page,
      row(page, BIDS.ambiguousRoot).locator("[data-move-handle]"),
      row(page, BIDS.ambiguousTarget),
      "inside",
    );
    await expect.poll(() => attempts, {
      timeout: 3_000,
      message: "relocation preflight should reach the durable move endpoint",
    }).toBe(1);
    await committed;
    expect(occurrences(await noteContent(request, SOURCE), BIDS.ambiguousRoot)).toBe(0);
    expect(occurrences(await noteContent(request, DESTINATION), BIDS.ambiguousRoot)).toBe(1);

    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "retryable");
    await expect(page.locator("[data-move-status='retryable']")).toContainText(/R or Enter/i);
    await expect(source.locator("[data-block-outliner]")).toHaveAttribute("inert", "");
    await expect(destination.locator("[data-block-outliner]")).toHaveAttribute("inert", "");
    expect(await propertyReservationHeld(page, SOURCE)).toBe(true);
    expect(await propertyReservationHeld(page, DESTINATION)).toBe(true);

    const pendingMarker = await page.evaluate(
      (key) => sessionStorage.getItem(key),
      RECOVERY_STORAGE_KEY,
    );
    expect(pendingMarker).not.toBeNull();
    expect((JSON.parse(pendingMarker!).request as unknown)).toEqual(JSON.parse(firstBody));

    await page.reload();
    await mountDay(page, SOURCE);
    await mountDay(page, DESTINATION);
    await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "retryable");
    expect(await propertyReservationHeld(page, SOURCE)).toBe(true);
    expect(await propertyReservationHeld(page, DESTINATION)).toBe(true);

    await page.keyboard.press("Enter");
    await expect.poll(() => attempts).toBe(2);
    await waitForMoveIdle(page);
    expect(secondBody).toBe(firstBody);
    await expect(source.locator(`[data-block-bid="${BIDS.ambiguousRoot}"]`)).toHaveCount(0);
    await expect(destination.locator(`[data-block-bid="${BIDS.ambiguousRoot}"]`)).toBeVisible();
    await expect(row(page, BIDS.ambiguousRoot).locator(".cm-content")).toBeFocused();
    expect(await propertyReservationHeld(page, SOURCE)).toBe(false);
    expect(await propertyReservationHeld(page, DESTINATION)).toBe(false);
    expect(await page.evaluate((key) => sessionStorage.getItem(key), RECOVERY_STORAGE_KEY)).toBeNull();
    await page.unroute(MOVE_ROUTE);
  });

  test("type immediately before pointer and Alt moves survives reload exactly once", async ({ page, request }) => {
    await openJournal(page);
    const pointerMarker = "__E2E_POINTER_RACE__";
    const altMarker = "__E2E_ALT_RACE__";

    await focusNormal(page, BIDS.racePointerRoot);
    await page.keyboard.press("Shift+A");
    await page.keyboard.type(pointerMarker);
    await dispatchToPlacement(
      page,
      row(page, BIDS.racePointerRoot).locator("[data-move-handle]"),
      row(page, BIDS.racePointerTarget),
      "inside",
    );
    await waitForMoveIdle(page);

    await focusNormal(page, BIDS.raceAltRoot);
    await page.keyboard.press("Shift+A");
    await page.keyboard.type(altMarker);
    await page.keyboard.press("Alt+ArrowDown");
    await waitForMoveIdle(page);

    await page.reload();
    await mountDay(page, SOURCE);
    await mountDay(page, DESTINATION);
    const sourceContent = await noteContent(request, SOURCE);
    const destinationContent = await noteContent(request, DESTINATION);
    expect(occurrences(sourceContent, pointerMarker)).toBe(0);
    expect(occurrences(destinationContent, pointerMarker)).toBe(1);
    expect(occurrences(sourceContent, altMarker)).toBe(1);
    expect(occurrences(destinationContent, altMarker)).toBe(0);
    await expect(row(page, BIDS.racePointerRoot).locator(".cm-content")).toContainText(pointerMarker);
    await expect(row(page, BIDS.raceAltRoot).locator(".cm-content")).toContainText(altMarker);
  });

  test("relocation waits for an in-flight structured property write", async ({ page, request }) => {
    const { source, destination } = await openJournal(page);
    let releaseWrites!: () => void;
    let markPropertyStarted!: () => void;
    const writeGate = new Promise<void>((resolve) => { releaseWrites = resolve; });
    const propertyStarted = new Promise<void>((resolve) => { markPropertyStarted = resolve; });
    let moveRequests = 0;
    const propertyValues: string[] = [];
    let propertyRequests = 0;
    let latePropertyRequests = 0;

    await page.route(SET_PROPERTY_ROUTE, async (route) => {
      const payload = route.request().postDataJSON() as {
        block_id?: string;
        key?: string;
        value?: string;
      };
      if (
        payload.block_id === `${SOURCE}:${BIDS.propertyRaceRoot}`
        && payload.key === "status"
      ) {
        propertyRequests++;
        propertyValues.push(payload.value ?? "");
        markPropertyStarted();
        if (propertyRequests === 1) await writeGate;
      }
      if (
        payload.block_id?.endsWith(`:${BIDS.propertyRaceRoot}`)
        && payload.key === "priority"
        && payload.value === "A"
      ) {
        latePropertyRequests++;
      }
      await route.continue();
    });
    await page.route(MOVE_ROUTE, async (route) => {
      moveRequests++;
      await writeGate;
      await route.continue();
    });

    try {
      const statusButton = row(page, BIDS.propertyRaceRoot).locator("button[title^='Status:']");
      await statusButton.click();
      await statusButton.click();
      await propertyStarted;
      await page.waitForTimeout(250);
      expect(propertyRequests).toBe(1);

      await dispatchToPlacement(
        page,
        row(page, BIDS.propertyRaceRoot).locator("[data-move-handle]"),
        row(page, BIDS.propertyRaceTarget),
        "after",
      );
      await expect(page.locator(".journal")).toHaveAttribute("data-move-mode", "pending");
      await page.waitForTimeout(250);
      expect(moveRequests).toBe(0);
      expect(await propertyReservationHeld(page, SOURCE)).toBe(true);
      expect(await propertyReservationHeld(page, DESTINATION)).toBe(true);

      const rejectedLateWrite = await setPropertyThroughApp(
        page,
        `${SOURCE}:${BIDS.propertyRaceRoot}`,
        "priority",
        "A",
      );
      expect(rejectedLateWrite.ok).toBe(false);
      expect(rejectedLateWrite.message).toMatch(/reserved for block relocation/i);
      expect(latePropertyRequests).toBe(0);

      releaseWrites();
      await waitForMoveIdle(page);
      expect(moveRequests).toBe(1);
      expect(propertyRequests).toBe(2);
      expect(propertyValues[0]).not.toBe(propertyValues.at(-1));
      await expect(source.locator(`[data-block-bid="${BIDS.propertyRaceRoot}"]`)).toHaveCount(0);
      await expect(destination.locator(`[data-block-bid="${BIDS.propertyRaceRoot}"]`)).toBeVisible();
      expect(await propertyReservationHeld(page, SOURCE)).toBe(false);
      expect(await propertyReservationHeld(page, DESTINATION)).toBe(false);

      const acceptedAfterMove = await setPropertyThroughApp(
        page,
        `${DESTINATION}:${BIDS.propertyRaceRoot}`,
        "priority",
        "A",
      );
      expect(acceptedAfterMove).toEqual({ ok: true, message: "" });
      expect(latePropertyRequests).toBe(1);

      const destinationContent = await noteContent(request, DESTINATION);
      const blockStart = destinationContent.indexOf(
        `- PROPERTY_RACE_ROOT <!-- bid:${BIDS.propertyRaceRoot} -->`,
      );
      expect(blockStart).toBeGreaterThanOrEqual(0);
      const nextRoot = destinationContent.indexOf("\n- ", blockStart + 1);
      const blockContent = destinationContent.slice(
        blockStart,
        nextRoot < 0 ? destinationContent.length : nextRoot,
      );
      expect(blockContent).toContain(`status:: ${propertyValues.at(-1)}`);
      expect(blockContent).toContain("priority:: A");
    } finally {
      releaseWrites();
    }
  });

  test("a failed property write blocks every relocation attempt until reload", async ({ page }) => {
    await openJournal(page);
    let propertyRequests = 0;
    let moveRequests = 0;

    await page.route(SET_PROPERTY_ROUTE, async (route) => {
      const payload = route.request().postDataJSON() as {
        block_id?: string;
        key?: string;
      };
      if (
        payload.block_id === `${SOURCE}:${BIDS.propertyFailureRoot}`
        && payload.key === "status"
      ) {
        propertyRequests++;
        await route.abort("failed");
        return;
      }
      await route.continue();
    });
    await page.route(MOVE_ROUTE, async (route) => {
      moveRequests++;
      await route.continue();
    });

    await row(page, BIDS.propertyFailureRoot).locator("button[title^='Status:']").click();
    await expect.poll(() => propertyRequests).toBe(1);

    for (let attempt = 0; attempt < 2; attempt++) {
      await dispatchToPlacement(
        page,
        row(page, BIDS.propertyFailureRoot).locator("[data-move-handle]"),
        row(page, BIDS.propertyFailureTarget),
        "after",
      );
      await waitForMoveIdle(page);
      expect(moveRequests).toBe(0);
      expect(await propertyReservationHeld(page, SOURCE)).toBe(false);
      expect(await propertyReservationHeld(page, DESTINATION)).toBe(false);
      await expect(page.locator(".tesela-toast")).toContainText(/property save.*uncertain.*reload/i);
    }
  });
});
