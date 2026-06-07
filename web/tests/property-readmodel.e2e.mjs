// Editor property/tag read-model regression (Graphite /g). Locks in two
// shipped fixes so they can't silently regress:
//
//   • ca39efa "preserve structured properties on edit" — handleBlockChange /
//     handleLoroText / bulk-tag re-derive ONLY tags+text and leave the
//     container-sourced block.properties untouched, so a container-set status
//     is NOT wiped on the next keystroke.
//   • f90eefe "#Task survives editing" — ⌘↵ adds the tag via a minimal splice
//     into the bound block's text_seq, so the next keystroke's tag re-derive
//     doesn't drop the chip.
//
// The flow needs only ⌘↵ (the cm6 Mod-Enter keymap → handleStatusCycle) and
// typing — both fire for synthetic Playwright keystrokes (the slash `/`
// inputHandler caveat does NOT apply here). Status survival is asserted on the
// materialized note on disk (exactly one `status::` line — not wiped, not
// duped); #Task survival is asserted via the `a[href="/p/task"]` pill.
//
// PREREQUISITES (same as the vim-*.e2e.mjs tests): a FRESH mosaic + a
// static-serving tesela-server on :7788 + `npx playwright install chromium`:
//   tesela init /tmp/a5qa
//   pnpm build
//   TESELA_SERVER_BIND=127.0.0.1:7788 TESELA_STATIC_DIR="$PWD/build" \
//     TESELA_DISABLE_MDNS=1 ../target/debug/tesela-server --mosaic /tmp/a5qa &
//   node tests/property-readmodel.e2e.mjs
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:7788/g';
const browser = await chromium.launch();
const ctx = await browser.newContext();
await ctx.addInitScript(() => {
  // Same-origin API base so both the app and our test fetch hit :7788.
  window.__TESELA_API_BASE__ = '';
});
const page = await ctx.newPage();
const errs = [];
page.on('pageerror', (e) => errs.push('PAGEERR ' + e.message));

await page.goto(URL, { waitUntil: 'domcontentloaded' });
await page.waitForSelector('.cm-editor', { timeout: 15000 });
await page.waitForTimeout(1500);

const results = [];
const check = (name, pass, detail) => results.push({ name, pass: !!pass, detail });

// Today's daily slug — the SAME local-date rule GraphiteShell/JournalView use,
// so it matches the note the fresh block lands in.
const d = new Date();
const slug = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

const taskPills = () =>
  page.evaluate(() => document.querySelectorAll('a[href="/p/task"]').length);
const noteContent = () =>
  page.evaluate(
    (s) =>
      fetch('/notes/' + s)
        .then((r) => (r.ok ? r.json() : null))
        .then((n) => (n ? n.content : null))
        .catch(() => null),
    slug,
  );
const esc = async () => {
  await page.keyboard.press('Escape');
  await page.waitForTimeout(60);
};
const freshBlock = async (text) => {
  await esc();
  await page.keyboard.press('o');
  await page.waitForTimeout(120);
  await page.keyboard.type(text, { delay: 14 });
  await page.waitForTimeout(120);
  await esc();
};

await page.click('.cm-content');
await page.waitForTimeout(250);

// ── Create a block and make it a task with ⌘↵ ───────────────────────────────
await freshBlock('buy milk');
await page.waitForTimeout(400);
await page.keyboard.press('Meta+Enter'); // ⌘↵ → status cycles to non-empty + auto #Task
await page.waitForTimeout(600);

check('⌘↵ adds the #Task pill', (await taskPills()) >= 1, await taskPills());
{
  const c = await noteContent();
  check('⌘↵ set exactly one status:: line', (c?.match(/status::/g) || []).length === 1, c);
  check('⌘↵ added exactly one tags:: Task line', (c?.match(/tags::\s*Task/gi) || []).length === 1, c);
}

// ── Type more text into the SAME block; status + #Task must survive ──────────
await page.keyboard.press('A'); // vim append → INSERT at end of line
await page.waitForTimeout(120);
await page.keyboard.type(' today', { delay: 14 });
await page.waitForTimeout(120);
await esc();
await page.waitForTimeout(900); // let the splice + debounced save flush + materialize

check('#Task pill SURVIVES a subsequent edit (f90eefe)', (await taskPills()) >= 1, await taskPills());
{
  const c = await noteContent();
  check(
    'status:: SURVIVES the edit — still exactly one line, not wiped/duped (ca39efa)',
    (c?.match(/status::/g) || []).length === 1,
    c,
  );
  check('tags:: Task still exactly one line after the edit', (c?.match(/tags::\s*Task/gi) || []).length === 1, c);
}

// ── Second status cycle + edit stays clean (no accumulation) ─────────────────
await page.keyboard.press('Meta+Enter'); // cycle again (todo → doing)
await page.waitForTimeout(600);
await page.keyboard.press('A');
await page.waitForTimeout(120);
await page.keyboard.type('!', { delay: 14 });
await esc();
await page.waitForTimeout(900);

check('2nd cycle: #Task pill still present', (await taskPills()) >= 1, await taskPills());
{
  const c = await noteContent();
  check('2nd cycle: still exactly one status:: line', (c?.match(/status::/g) || []).length === 1, c);
  check('2nd cycle: still exactly one tags:: Task line', (c?.match(/tags::\s*Task/gi) || []).length === 1, c);
}

console.log('=== PAGE ERRORS (' + errs.length + ') ===');
for (const e of errs) console.log(e);
console.log('=== RESULTS ===');
let passN = 0;
for (const r of results) {
  console.log(`${r.pass ? 'PASS' : 'FAIL'}  ${r.name}${r.pass ? '' : '   got=' + JSON.stringify(r.detail)}`);
  if (r.pass) passN++;
}
console.log(`\n${passN}/${results.length} passed`);
await browser.close();
process.exit(passN === results.length ? 0 : 1);
