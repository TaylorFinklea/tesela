// Model B — configurable NL triggers + lift-on-blur (Graphite /g).
//   • make-task (⌘↵) just TAGS — it does NOT lift while you're still focused.
//   • leaving the block (blur) lifts detected tokens into structured props.
//   • config-driven: Priority (select p1-p4), Deadline (date, "due"/"deadline",
//     keeps time), Scheduled (default date prop), Points (number, "points"/"pts").
//   • gating: non-Task blocks detect nothing.
//
// PREREQS: vite on :5173 + a fresh tesela-server on :7474 whose seeded property
// pages carry nl_triggers (server build ≥ 2026-06-08) + playwright.
// Run: REPRO_URL=http://127.0.0.1:5173/g node tests/task-tokens-nlp.e2e.mjs
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:5173/g';
const browser = await chromium.launch();
const page = await (await browser.newContext()).newPage();
const errs = [];
page.on('pageerror', (e) => errs.push('PAGEERR ' + e.message));
await page.goto(URL, { waitUntil: 'domcontentloaded' });
await page.waitForSelector('.cm-editor', { timeout: 15000 });
await page.waitForTimeout(1800); // let allNotes (incl. the property pages) load

const results = [];
const check = (n, p, d) => results.push({ n, p: !!p, d });
const dt = new Date();
const slug = `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}-${String(dt.getDate()).padStart(2, '0')}`;
const noteContent = () =>
  page.evaluate((s) => fetch('/api/notes/' + s).then((r) => (r.ok ? r.json() : null)).then((n) => (n ? n.content : null)).catch(() => null), slug);
const esc = async () => { await page.keyboard.press('Escape'); await page.waitForTimeout(80); };
const blurEditor = async () => { await page.evaluate(() => (document.querySelector('.cm-content'))?.blur()); await page.waitForTimeout(1800); };
const makeTaskBlock = async (text) => {
  await esc();
  await page.keyboard.press('o');
  await page.waitForTimeout(150);
  await page.keyboard.type(text, { delay: 16 });
  await page.waitForTimeout(1600); // auto-save so the block's bid resolves
  await page.keyboard.press('Meta+Enter'); // make-task = tag only (no lift yet)
  await page.waitForTimeout(600);
};
const ISO = '\\d{4}-\\d{2}-\\d{2}';

await page.click('.cm-content');
await page.waitForTimeout(250);

// 1) make-task does NOT lift while focused
await makeTaskBlock('buy milk due fri p1');
const duringFocus = await page.evaluate(() => document.body.innerText);
check('make-task does NOT lift while focused (tokens stay in prose)', /due fri/i.test(duringFocus) && /\bp1\b/i.test(duringFocus), duringFocus.slice(0, 120));
await blurEditor();

// 2) due + TIME
await makeTaskBlock('call vet due thu at 8');
await blurEditor();

// 3) number property (Points) — "5 points" + bare date → default (scheduled)
await makeTaskBlock('fold laundry tom 5 points');
await blurEditor();

// 3b) multi-word bare date → default (scheduled): "next tuesday"
await makeTaskBlock('email boss next tuesday');
await blurEditor();

// 4) gating: a NON-task block detects nothing
await esc();
await page.keyboard.press('o');
await page.waitForTimeout(150);
await page.keyboard.type('note p1 due fri', { delay: 16 });
await page.waitForTimeout(1600);
await blurEditor();

// Settle past all the async lift → container-op → materialize round-trips,
// then assert the FINAL on-disk state of every block.
await page.waitForTimeout(2500);
const c = (await noteContent()) || '';
check('blur lifted deadline (due fri → date)', /buy milk[\s\S]*?deadline::\s*\[?\[?2026-/.test(c) || new RegExp('deadline::\\s*\\[?\\[?' + ISO).test(c), c);
check('blur lifted priority p1', /priority::\s*p1/i.test(c), c);
check('"buy milk" kept, "due fri"/"p1" stripped from it', /(^|\n)\s*-\s*buy milk\s/i.test(c) && !/buy milk due fri/i.test(c), c);
check('deadline keeps the time ("thu at 8")', /deadline::\s*\[?\[?\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}/.test(c), c);
check('points:: 5 lifted (number trigger)', /points::\s*5\b/i.test(c), c);
check('scheduled lifted (bare "tom" → default date prop)', new RegExp('scheduled::\\s*\\[?\\[?' + ISO).test(c), c);
check('"fold laundry" kept, "5 points" stripped', /(^|\n)\s*-\s*fold laundry\s/i.test(c) && !/5 points/i.test(c), c);
check('multi-word date "next tuesday" → detected + stripped', /(^|\n)\s*-\s*email boss\s/i.test(c) && !/next tuesday/i.test(c), c);
check('non-task block: "p1 due fri" NOT detected (stays prose)', /note p1 due fri/i.test(c), c);

console.log('PAGE ERRORS:', errs.length, errs.slice(0, 4));
let pass = 0;
for (const r of results) {
  console.log(`${r.p ? 'PASS' : 'FAIL'}  ${r.n}${r.p ? '' : '   got=' + JSON.stringify(r.d)}`);
  if (r.p) pass++;
}
console.log(`\n${pass}/${results.length} passed`);
if (process.env.SHOT) await page.screenshot({ path: process.env.SHOT }).catch(() => {});
await browser.close();
process.exit(pass === results.length && errs.length === 0 ? 0 : 1);
