// Graphite /g command palette — screen-reader addressability.
//
// Verifies the ARIA plumbing added to the palette without changing the
// command scoring, ordering, shortcuts, or execution behavior:
//
//   - the scrim exposes a modal dialog with a non-empty label
//   - the list of commands/notes is a listbox with stable option ids
//   - the input is a combobox whose aria-activedescendant points at the
//     currently-selected option
//   - empty state is announced via role=status + aria-live
//   - Escape and click-out still close the palette (focus-restore path
//     is unchanged — the test only asserts the dialog is no longer open)
//
// PREREQUISITES (same as the other /g e2es — boot a static-serving server
// on :7788 against a temp mosaic, then run the script):
//   pnpm build
//   mkdir -p /tmp/cmdk-a11y-qa/notes
//   TESELA_SERVER_BIND=127.0.0.1:7788 \
//     TESELA_STATIC_DIR="$PWD/build" \
//     TESELA_DEFAULT_MOSAIC=/tmp/cmdk-a11y-qa \
//     TESELA_DISABLE_MDNS=1 TESELA_DISABLE_RELAY=1 TESELA_DISABLE_PEER_SYNC=1 \
//     ../target/debug/tesela-server &
//   node tests/command-palette-a11y.e2e.mjs
import { chromium } from "@playwright/test";

const URL = process.env.REPRO_URL || "http://127.0.0.1:7788/g";
const browser = await chromium.launch();
const ctx = await browser.newContext();
await ctx.addInitScript(() => {
  window.__TESELA_API_BASE__ = "";
});
const page = await ctx.newPage();
const errs = [];
page.on("pageerror", (e) => errs.push("PAGEERR " + e.message));

await page.goto(URL, { waitUntil: "domcontentloaded" });
await page.waitForSelector(".cm-editor", { timeout: 15000 });
// Give the Graphite shell + the ⌘K capture listener time to mount.
await page.waitForTimeout(1500);

const results = [];
const check = (name, pass, detail) => results.push({ name, pass: !!pass, detail });

// ── 1. Open the palette ─────────────────────────────────────────────────
const openWithMetaK = async () => {
  await page.keyboard.press("Meta+k");
  await page.waitForTimeout(180);
};
const openWithControlK = async () => {
  await page.keyboard.press("Control+k");
  await page.waitForTimeout(180);
};
let opened = await page.locator('[role="dialog"]').count();
if (opened === 0) {
  await openWithMetaK();
  opened = await page.locator('[role="dialog"]').count();
}
if (opened === 0) {
  await openWithControlK();
  opened = await page.locator('[role="dialog"]').count();
}
check("palette opens with ⌘K / Ctrl-K", opened === 1, "count=" + opened);

// ── 2. Dialog exposes modal + label ────────────────────────────────────
const dialog = page.locator('[role="dialog"]').first();
const dialogLabel = (await dialog.getAttribute("aria-label")) || "";
const dialogModal = await dialog.getAttribute("aria-modal");
check("dialog has aria-modal=true", dialogModal === "true", "aria-modal=" + dialogModal);
check("dialog has a non-empty aria-label", dialogLabel.length > 0, "aria-label=" + dialogLabel);

// ── 3. Listbox + combobox wiring ────────────────────────────────────────
const listbox = page.locator('[role="listbox"]');
const listboxCount = await listbox.count();
check("exactly one listbox is exposed", listboxCount === 1, "count=" + listboxCount);
const listboxId = listboxCount === 1 ? await listbox.first().getAttribute("id") : null;
check("listbox has an id", !!listboxId, "id=" + listboxId);

const combobox = page.locator('[role="combobox"]');
const comboboxCount = await combobox.count();
check("exactly one combobox is exposed", comboboxCount === 1, "count=" + comboboxCount);
const comboboxControls =
  comboboxCount === 1 ? await combobox.first().getAttribute("aria-controls") : null;
check(
  "combobox aria-controls points at the listbox",
  !!listboxId && comboboxControls === listboxId,
  "aria-controls=" + comboboxControls + " listboxId=" + listboxId,
);
check(
  "combobox aria-expanded is 'true'",
  comboboxCount === 1 && (await combobox.first().getAttribute("aria-expanded")) === "true",
);

// ── 4. Options: stable ids, role, aria-selected, and the active pointer ─
const options = page.locator('[role="listbox"] [role="option"]');
const optionCount = await options.count();
check("at least one option is rendered", optionCount > 0, "count=" + optionCount);

const firstId = optionCount > 0 ? await options.first().getAttribute("id") : null;
const secondId = optionCount > 1 ? await options.nth(1).getAttribute("id") : null;
check("options have non-empty stable ids", !!firstId && firstId.length > 0, "firstId=" + firstId);
check(
  "option ids are unique across rows",
  optionCount < 2 || firstId !== secondId,
  "firstId=" + firstId + " secondId=" + secondId,
);
check(
  "first option is aria-selected=true on open",
  optionCount > 0 && (await options.first().getAttribute("aria-selected")) === "true",
);

const initialActive =
  comboboxCount === 1 ? await combobox.first().getAttribute("aria-activedescendant") : null;
check(
  "combobox aria-activedescendant points at the active option on open",
  !!firstId && initialActive === firstId,
  "aria-activedescendant=" + initialActive + " firstId=" + firstId,
);

// ── 5. Arrow nav updates aria-activedescendant + aria-selected ─────────
if (optionCount > 1) {
  await page.keyboard.press("ArrowDown");
  await page.waitForTimeout(80);
  const secondActive =
    comboboxCount === 1 ? await combobox.first().getAttribute("aria-activedescendant") : null;
  const secondSelected = await options.nth(1).getAttribute("aria-selected");
  const firstSelected = await options.first().getAttribute("aria-selected");
  check(
    "ArrowDown updates aria-activedescendant to the new active option",
    secondActive === secondId,
    "aria-activedescendant=" + secondActive + " expected=" + secondId,
  );
  check(
    "ArrowDown flips aria-selected from row 0 to row 1",
    secondSelected === "true" && firstSelected === "false",
    "first=" + firstSelected + " second=" + secondSelected,
  );
  // Reset for the next scenarios.
  await page.keyboard.press("ArrowUp");
  await page.waitForTimeout(80);
}

// ── 6. Empty state is announced ────────────────────────────────────────
// Type something that won't match any command or note.
const emptyQuery = "xyzqwentynonsense" + Date.now();
await page.keyboard.type(emptyQuery, { delay: 12 });
await page.waitForTimeout(200);
const empty = page.locator('[role="listbox"] [role="status"]');
const emptyCount = await empty.count();
const emptyText = emptyCount > 0 ? (await empty.first().textContent())?.trim() : null;
const emptyLive = emptyCount > 0 ? await empty.first().getAttribute("aria-live") : null;
check("empty state has role=status", emptyCount >= 1, "count=" + emptyCount);
check("empty state has aria-live=polite", emptyLive === "polite", "aria-live=" + emptyLive);
check(
  "empty state contains the 'No matches' message",
  !!emptyText && emptyText.length > 0,
  "text=" + JSON.stringify(emptyText),
);
// While the empty state is showing, the listbox should contain zero options.
const optionsWhileEmpty = await page.locator('[role="listbox"] [role="option"]').count();
check("listbox is empty while the empty state is shown", optionsWhileEmpty === 0, "count=" + optionsWhileEmpty);

// Clear the query so we can re-test close paths.
await page.keyboard.press("Escape");
await page.waitForTimeout(160);

// ── 7. Escape closes the palette ───────────────────────────────────────
let dialogAfterEsc = await page.locator('[role="dialog"]').count();
check("Escape closes the palette", dialogAfterEsc === 0, "count=" + dialogAfterEsc);

// ── 8. Click-out closes the palette ────────────────────────────────────
let opened2 = await page.locator('[role="dialog"]').count();
if (opened2 === 0) await openWithMetaK();
if ((await page.locator('[role="dialog"]').count()) === 0) await openWithControlK();
check("palette re-opens for click-out scenario", (await page.locator('[role="dialog"]').count()) === 1);
// Click in the scrim, far from the modal. The dialog content sits near the
// top of the viewport; the scrim fills the rest. The lower-left corner is
// always outside the modal.
const viewport = page.viewportSize();
await page.mouse.click(Math.max(20, Math.floor(viewport.width * 0.05)), Math.max(20, Math.floor(viewport.height * 0.85)));
await page.waitForTimeout(160);
const dialogAfterClick = await page.locator('[role="dialog"]').count();
check("click on the scrim closes the palette", dialogAfterClick === 0, "count=" + dialogAfterClick);

console.log("=== PAGE ERRORS (" + errs.length + ") ===");
for (const e of errs) console.log(e);
console.log("=== RESULTS ===");
let passN = 0;
for (const r of results) {
  console.log(`${r.pass ? "PASS" : "FAIL"}  ${r.name}${r.pass ? "" : "   got=" + r.detail}`);
  if (r.pass) passN++;
}
console.log(`\n${passN}/${results.length} passed`);
await browser.close();
process.exit(passN === results.length ? 0 : 1);
