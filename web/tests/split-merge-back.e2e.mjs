// Split + immediate merge-back regression (Graphite /g).
//
// Repro fixed by this test:
//   1. Enter-split a block.
//   2. Wait past the 500 ms block-op debounce so the new half may exist on disk.
//   3. Backspace-merge the new half into the previous block.
//   4. The absorbed half's bid must be deleted; it must not survive as a
//      duplicate second block in the materialized note.
//
// PREREQUISITES: static-serving tesela-server on :7788, same as the vim e2es:
//   pnpm build
//   TESELA_SERVER_BIND=127.0.0.1:7788 TESELA_STATIC_DIR="$PWD/build" \
//     TESELA_DISABLE_MDNS=1 TESELA_DISABLE_RELAY=1 TESELA_DISABLE_PEER_SYNC=1 \
//     ../target/debug/tesela-server --mosaic /tmp/split-merge-qa &
//   node tests/split-merge-back.e2e.mjs
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

const d = new Date();
const slug = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const nonce = `sm${Date.now()}`;
const left = `alpha_${nonce}`;
const right = `beta_${nonce}`;

const noteContent = () =>
  page.evaluate(
    (s) =>
      fetch("/notes/" + s)
        .then((r) => (r.ok ? r.json() : null))
        .then((n) => (n ? n.content : null))
        .catch(() => null),
    slug,
  );
const markerLines = async () => {
  const c = await noteContent();
  return (c || "")
    .split("\n")
    .filter((line) => /^\s*-\s/.test(line) && line.includes(nonce));
};
const vim = async (seq) => {
  for (const ch of seq) {
    await page.keyboard.press(ch);
    await page.waitForTimeout(45);
  }
};
const esc = async () => {
  await page.keyboard.press("Escape");
  await page.waitForTimeout(60);
};
const freshBlock = async (text) => {
  await esc();
  await page.keyboard.press("o");
  await page.waitForTimeout(120);
  await page.keyboard.type(text, { delay: 14 });
  await page.waitForTimeout(120);
  await esc();
};

await page.click(".cm-content");
await page.waitForTimeout(250);

await freshBlock(`${left} ${right}`);
await page.waitForTimeout(700);

// Move to the second word, enter insert mode there, and split before it.
await vim("0");
await vim("w");
await page.keyboard.press("i");
await page.waitForTimeout(80);
await page.keyboard.press("Enter");
await page.waitForTimeout(900);

let lines = await markerLines();
check("split created two marked blocks before merge", lines.length === 2, lines);

// The split's new block mounts focused in INSERT at cursor 0. Backspace merges
// it back into the previous block.
await page.keyboard.press("Backspace");
await page.waitForTimeout(1100);

lines = await markerLines();
check("merge leaves exactly one marked block on disk", lines.length === 1, lines);
check(
  "merged block contains both halves",
  lines.length === 1 && lines[0].includes(left) && lines[0].includes(right),
  lines,
);

console.log("=== PAGE ERRORS (" + errs.length + ") ===");
for (const e of errs) console.log(e);
console.log("=== RESULTS ===");
let passN = 0;
for (const r of results) {
  console.log(`${r.pass ? "PASS" : "FAIL"}  ${r.name}${r.pass ? "" : "   got=" + JSON.stringify(r.detail)}`);
  if (r.pass) passN++;
}
console.log(`\n${passN}/${results.length} passed`);
await browser.close();
process.exit(passN === results.length ? 0 : 1);
