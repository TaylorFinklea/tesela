// Model B Part 2a regression (Graphite /g): inline priority detect + lift.
//   • typing `p1`..`p3` highlights the token inline (cm-tesela-priority);
//   • pressing Enter at the end of the prose line LIFTS it to a structured
//     priority property (→ a ⚑P1 flag in the below-strip) and STRIPS `p1` from
//     the prose. The write is a structured container op (no `priority::` text
//     line dual-write).
//
// PREREQS: vite dev on :5173 + a fresh tesela-server on :7474 + playwright.
// Run:  REPRO_URL=http://127.0.0.1:5173/g node tests/task-tokens.e2e.mjs
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:5173/g';
const browser = await chromium.launch();
const page = await (await browser.newContext()).newPage();
const errs = [];
page.on('pageerror', (e) => errs.push('PAGEERR ' + e.message));
await page.goto(URL, { waitUntil: 'domcontentloaded' });
await page.waitForSelector('.cm-editor', { timeout: 15000 });
await page.waitForTimeout(1500);

const results = [];
const check = (n, p, d) => results.push({ n, p: !!p, d });
const dt = new Date();
const slug = `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}-${String(dt.getDate()).padStart(2, '0')}`;
const noteContent = () =>
  page.evaluate((s) => fetch('/api/notes/' + s).then((r) => (r.ok ? r.json() : null)).then((n) => (n ? n.content : null)).catch(() => null), slug);
const esc = async () => { await page.keyboard.press('Escape'); await page.waitForTimeout(60); };

await page.click('.cm-content');
await page.waitForTimeout(250);
await esc();
await page.keyboard.press('o');
await page.waitForTimeout(150);
await page.keyboard.type('ship the build p1', { delay: 18 });
await page.waitForTimeout(350);

// (1) inline highlight while typing
const hl = await page.evaluate(() => {
  const el = document.querySelector('.cm-tesela-priority');
  return el ? { text: el.textContent, color: getComputedStyle(el).color } : null;
});
check('p1 highlighted inline while typing', hl && /p1/i.test(hl.text || ''), hl);
check('the inline highlight is P1 red', hl && /235,\s*92,\s*88/.test(hl.color || ''), hl?.color);

// let the block's text auto-save (so its bid is server-resolved) before the
// lift fires its container op — mirrors real typing cadence
await page.waitForTimeout(1600);

// (2) commit with Enter → lift
await page.keyboard.press('Enter');
await page.waitForTimeout(1500);
await esc();
await page.waitForTimeout(900);

const after = await page.evaluate(() => {
  const flag = [...document.querySelectorAll('span, button')].find((e) => /P1/.test(e.textContent || '') && /⚑/.test(e.textContent || ''));
  return {
    flag: flag ? flag.textContent.replace(/\s+/g, ' ').trim() : null,
    flagColor: flag ? getComputedStyle(flag).color : null,
    proseStillHasToken: /ship the build p1/i.test(document.body.innerText),
  };
});
check('lift → ⚑P1 flag shows in the strip', after.flag && /P1/.test(after.flag), after);
check('"p1" stripped from the prose (DOM)', !after.proseStillHasToken, after);

const c = await noteContent();
check('priority:: p1 materialized (structured op)', /priority::\s*p1/i.test(c || ''), c);
check('prose line is "ship the build" (no inline p1)', /(^|\n)\s*-?\s*ship the build(\s|$)/i.test(c || '') && !/ship the build p1/i.test(c || ''), c);

console.log('PAGE ERRORS:', errs.length, errs.slice(0, 3));
let pass = 0;
for (const r of results) {
  console.log(`${r.p ? 'PASS' : 'FAIL'}  ${r.n}${r.p ? '' : '   got=' + JSON.stringify(r.d)}`);
  if (r.p) pass++;
}
console.log(`\n${pass}/${results.length} passed`);
if (process.env.SHOT) await page.screenshot({ path: process.env.SHOT }).catch(() => {});
await browser.close();
process.exit(pass === results.length && errs.length === 0 ? 0 : 1);
