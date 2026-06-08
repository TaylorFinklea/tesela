// Tag redesign regression (Graphite /g) — Model A (decided 2026-06-07):
//   • a #tag becomes a right-edge COLORED pill ONLY when ↵-committed (it lands
//     on the block's `tags::` line); the pill is colored per-tag.
//   • ⌘↵ on the autocomplete keeps the tag INLINE (#text in the prose) — no
//     pill, no `tags::` line.
//
// Structure: create all the blocks first (asserting the COMMIT result — on-disk
// markdown + ⌘↵ inline-ness — inline, which is race-free), THEN reload once and
// assert the RENDERED pills/colors from the durable on-disk state. The reload
// sidesteps a pre-existing optimistic-vs-refetch race: committing a chip tag now
// materializes a Tag page → a broad notes refetch that can transiently clobber a
// just-written pill in the in-memory journal cache (the on-disk truth is always
// correct, so a reload renders it).
//
// Drives the #tag autocomplete with REAL keystrokes (the only thing that fires
// cm6's #-inputHandler). Runs against the vite dev server (default /api proxy).
//
// PREREQUISITES: vite dev on :5173 (pnpm dev) + a fresh tesela-server on :7474
// (`tesela init /tmp/x && tesela-server --mosaic /tmp/x`) + playwright chromium.
// Run:  REPRO_URL=http://127.0.0.1:5173/g node tests/tag-redesign.e2e.mjs
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:5173/g';
const browser = await chromium.launch();
const ctx = await browser.newContext();
const page = await ctx.newPage();
const errs = [];
page.on('pageerror', (e) => errs.push('PAGEERR ' + e.message));

await page.goto(URL, { waitUntil: 'domcontentloaded' });
await page.waitForSelector('.cm-editor', { timeout: 15000 });
await page.waitForTimeout(1500);

const results = [];
const check = (name, pass, detail) => results.push({ name, pass: !!pass, detail });

const d = new Date();
const slug = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
const noteContent = () =>
  page.evaluate(
    (s) => fetch('/api/notes/' + s).then((r) => (r.ok ? r.json() : null)).then((n) => (n ? n.content : null)).catch(() => null),
    slug,
  );
const pill = (name) =>
  page.evaluate((n) => {
    const a = document.querySelector(`a[href="/p/${n}"]`);
    if (!a) return null;
    const span = a.closest('span');
    return { present: true, style: span?.getAttribute('style') || '' };
  }, name);
const colorOf = (name) =>
  page.evaluate((n) => {
    const a = document.querySelector(`a[href="/p/${n}"]`);
    const dot = a?.closest('span')?.querySelector('span[style*="background"]');
    return dot?.getAttribute('style') || null;
  }, name);
const esc = async () => { await page.keyboard.press('Escape'); await page.waitForTimeout(60); };
const freshBlock = async (text) => {
  await esc();
  await page.keyboard.press('o');
  await page.waitForTimeout(120);
  await page.keyboard.type(text, { delay: 16 });
  await page.waitForTimeout(150);
};

await page.click('.cm-content');
await page.waitForTimeout(250);

// ── Create the blocks + assert the COMMIT result (on-disk / inline) ─────────
// errand — ↵ commits to a chip (tags:: line)
await freshBlock('buy oat milk #errand');
await page.waitForTimeout(400);
await page.keyboard.press('Enter');
await esc();
await page.waitForTimeout(900);
{
  const c = await noteContent();
  check('↵ wrote a tags:: line (chip), case-insensitive', /tags::.*errand/i.test(c || ''), c);
  check('↵ left NO inline #errand in the prose', !/#errand/i.test(c || ''), c);
}

// article — ⌘↵ keeps it inline (no chip)
await freshBlock('read the #article');
await page.waitForTimeout(400);
await page.keyboard.press('Meta+Enter');
await esc();
await page.waitForTimeout(900);
{
  const c = await noteContent();
  check('⌘↵ kept #article inline in the prose', /#article/i.test(c || ''), c);
  check('⌘↵ wrote NO tags:: article line', !/tags::.*article/i.test(c || ''), c);
}

// urgent — ↵ chip (second distinct tag for the color check)
await freshBlock('call the plumber #urgent');
await page.waitForTimeout(400);
await page.keyboard.press('Enter');
await esc();
await page.waitForTimeout(900);

// wash — ⌘↵ with NO popup still = make-task (the Mod-Enter guard must yield)
await freshBlock('wash the car');
await page.waitForTimeout(300);
await page.keyboard.press('Meta+Enter');
await esc();
await page.waitForTimeout(900);
{
  const c = await noteContent();
  check('⌘↵ (no popup) make-task wrote a tags:: Task chip line', /tags::.*task/i.test(c || ''), c);
}

// ── Reload → assert the RENDERED pills from the durable on-disk state ────────
await page.reload({ waitUntil: 'domcontentloaded' });
await page.waitForSelector('.cm-editor', { timeout: 15000 });
await page.waitForTimeout(2000);

const errandPill = await pill('errand');
check('#errand renders a colored pill', !!errandPill?.present && /background|color-mix/.test(errandPill?.style || ''), errandPill);
check('#article (⌘↵ inline) renders NO pill', !(await pill('article'))?.present, await pill('article'));
check('#urgent renders a pill', !!(await pill('urgent'))?.present, await pill('urgent'));
check('#Task (make-task) renders a pill', !!(await pill('task'))?.present, await pill('task'));
{
  const cErrand = await colorOf('errand');
  const cUrgent = await colorOf('urgent');
  check('two distinct tags render distinct dot colors', !!cErrand && !!cUrgent && cErrand !== cUrgent, { cErrand, cUrgent });
}

await page.screenshot({ path: process.env.SHOT || '/tmp/tag-redesign-shot.png' }).catch(() => {});

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
