// j/k navigation must not unexpectedly enter INSERT mode.
//
// Regression target: one-shot "start in insert" hints for newly-created or
// split blocks must be consumed by the creation focus only. Later j/k
// navigation back onto that block should land in NORMAL mode — across every
// focus path the user can take (Esc transitions, command palette open/close,
// quick-capture `:` open/close, blur-via-click and refocus).
//
// PREREQUISITES: static-serving tesela-server on :7788, same as the vim e2es:
//   pnpm build
//   TESELA_SERVER_BIND=127.0.0.1:7788 TESELA_STATIC_DIR="$PWD/build" \
//     TESELA_DISABLE_MDNS=1 TESELA_DISABLE_RELAY=1 TESELA_DISABLE_PEER_SYNC=1 \
//     ../target/debug/tesela-server --mosaic /tmp/jk-normal-qa &
//   node tests/jk-normal-mode.e2e.mjs
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
await page.waitForTimeout(1500);

const results = [];
const check = (name, pass, detail) => results.push({ name, pass: !!pass, detail });

const focused = () =>
  page.evaluate(() => {
    const el = document.querySelector(".cm-editor.cm-focused");
    return {
      txt: el?.querySelector(".cm-content")?.innerText.replace(/\n+$/, "") ?? null,
      normal: !!el?.querySelector(".cm-fat-cursor"),
    };
  });
const vim = async (seq) => {
  for (const ch of seq) {
    await page.keyboard.press(ch);
    await page.waitForTimeout(45);
  }
};
const esc = async () => {
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
};
const freshBlock = async (text) => {
  await esc();
  await page.keyboard.press("o");
  await page.waitForTimeout(120);
  if (text) await page.keyboard.type(text, { delay: 14 });
  await page.waitForTimeout(120);
  await esc();
};

await page.click(".cm-content");
await page.waitForTimeout(250);

// Scenario 1: a newly-created empty block. It should auto-enter insert once,
// but after Escape, k then j back onto it must leave it in NORMAL.
await freshBlock("anchor before empty");
await page.keyboard.press("o");
await page.waitForTimeout(160);
await esc();
await vim("k");
await vim("j");
let s = await focused();
check("j back onto a freshly-created empty block stays NORMAL", s.normal && s.txt === "", JSON.stringify(s));

// Scenario 2: an Enter split with text after the cursor. The split half starts
// in insert once; later k/j navigation back onto it must not re-enter insert.
await freshBlock("split_left split_right");
await vim("0");
await vim("w");
await page.keyboard.press("i");
await page.waitForTimeout(80);
await page.keyboard.press("Enter");
await page.waitForTimeout(200);
await esc();
await vim("k");
await vim("j");
s = await focused();
check(
  "j back onto an Enter-split block stays NORMAL",
  s.normal && s.txt === "split_right",
  JSON.stringify(s),
);

// Scenario 3: ⌘K command palette open/close must not push focus back into
// INSERT. The palette is a modal dialog that steals DOM focus; when it
// closes, focus returns to the cm-editor and the focused block's effect
// re-runs. Without the `appliedAutoInsert` one-shot gate + the parent's
// hint-consume, j/k after the close would land the user in INSERT.
const openPalette = async () => {
  await page.keyboard.press("Meta+k");
  await page.waitForTimeout(200);
  let n = await page.locator('[role="dialog"]').count();
  if (n === 0) {
    await esc();
    await page.waitForTimeout(80);
    await page.keyboard.press("Control+k");
    await page.waitForTimeout(200);
    n = await page.locator('[role="dialog"]').count();
  }
  return n;
};
let dialogN = await openPalette();
check("⌘K opens the command palette", dialogN === 1, "count=" + dialogN);
await page.keyboard.press("Escape");
await page.waitForTimeout(240);
s = await focused();
check("cm-editor still focused after ⌘K close", s.txt !== null, JSON.stringify(s));
check("cm-editor in NORMAL after ⌘K close", s.normal, JSON.stringify(s));
await vim("j");
s = await focused();
check("j after ⌘K close stays in NORMAL", s.normal, JSON.stringify(s));

// Scenario 4: `:` quick-capture (colon command line) open/close must not push
// focus back into INSERT. Same shape as ⌘K, but for the `:` ex-mode input
// reachable from the rail "Quick capture" widget AND from NORMAL-mode `:`.
// NOTE: the colon command line currently does NOT auto-restore DOM focus
// to the cm-editor on close (focus falls back to <body>). The test
// simulates the user's next action — clicking back on .cm-content — so
// the j/k-after-focus-steal path is what the "stale insert intent" fix
// actually owns. Follow-up: restore focus in ColonCommandLine on close.
await esc();
await page.waitForTimeout(80);
let cmFocusedBefore = await page.evaluate(() => !!document.querySelector(".cm-editor.cm-focused"));
check("cm-editor focused before `:`", cmFocusedBefore, "cm-focused=" + cmFocusedBefore);
await page.keyboard.press(":");
await page.waitForTimeout(200);
let cmFocusedDuringColon = await page.evaluate(() => !!document.querySelector(".cm-editor.cm-focused"));
check("cm-editor loses DOM focus while `:` input is open", !cmFocusedDuringColon, "cm-focused=" + cmFocusedDuringColon);
await page.keyboard.press("Escape");
await page.waitForTimeout(240);
// The colon command line does not auto-restore focus (known follow-up).
// Refocus the cm-editor the way the user would — by clicking it.
await page.click(".cm-content");
await page.waitForTimeout(220);
s = await focused();
check("cm-editor refocused after `:` close + click", s.txt !== null, JSON.stringify(s));
check("cm-editor in NORMAL after `:` close + click", s.normal, JSON.stringify(s));
await vim("j");
s = await focused();
check("j after `:` close + refocus stays in NORMAL", s.normal, JSON.stringify(s));

// Scenario 5: blur cm-editor via click on a non-editor element, then
// refocus by clicking back on the cm-content, then j/k. Tests that the
// effect that re-fires on `view.hasFocus` flip → true does not push the
// user back into INSERT.
await esc();
await page.waitForTimeout(80);
let cmFocusedBeforeBlur = await page.evaluate(() => !!document.querySelector(".cm-editor.cm-focused"));
check("cm-editor focused before blur click", cmFocusedBeforeBlur, "cm-focused=" + cmFocusedBeforeBlur);
// Click on a non-editor surface. The status bar is a non-focusable div at
// the bottom of the shell — clicking it moves the mouse outside any editor
// and the cm-editor's contenteditable loses DOM focus.
await page.locator(".gr-status").click();
await page.waitForTimeout(180);
let cmFocusedAfterBlur = await page.evaluate(() => !!document.querySelector(".cm-editor.cm-focused"));
check("cm-editor blurred after click on status bar", !cmFocusedAfterBlur, "cm-focused=" + cmFocusedAfterBlur);
// Refocus by clicking the cm-content. The focusedIndex prop is unchanged
// (the click is inside an existing block), so only the DOM focus flips.
await page.click(".cm-content");
await page.waitForTimeout(220);
s = await focused();
check("cm-editor refocused after click", s.txt !== null, JSON.stringify(s));
check("cm-editor in NORMAL after re-focus", s.normal, JSON.stringify(s));
await vim("j");
s = await focused();
check("j after blur+refocus stays in NORMAL", s.normal, JSON.stringify(s));

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
