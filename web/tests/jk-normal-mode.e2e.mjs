// j/k navigation must not unexpectedly enter INSERT mode.
//
// Regression target: one-shot "start in insert" hints for newly-created or
// split blocks must be consumed by the creation focus only. Later j/k
// navigation back onto that block should land in NORMAL mode.
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
