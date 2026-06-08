// Model B — the Enter-lift path (Graphite /g), distinct from the ⌘↵ make-task
// path covered by task-tokens-2b.e2e.mjs. On a block that ALREADY carries a
// detect-enabled tag (#Task), pressing Enter at the end of the prose line lifts
// detected priority into a structured property + strips it. Also asserts the
// inline highlight is GATED (only lights up once the block has #Task).
//
// PREREQS: vite dev on :5173 + a fresh tesela-server on :7474 + playwright.
// Run: REPRO_URL=http://127.0.0.1:5173/g node tests/task-tokens.e2e.mjs
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
const esc = async () => { await page.keyboard.press('Escape'); await page.waitForTimeout(80); };

await page.click('.cm-content');
await page.waitForTimeout(250);

// type a block with priority + an inline #Task; the #Task makes the block
// detection-enabled, so p2 should highlight inline.
await esc();
await page.keyboard.press('o');
await page.waitForTimeout(150);
await page.keyboard.type('ship build p2 ', { delay: 16 });
await page.keyboard.type('#task', { delay: 16 }); // opens the tag autocomplete
await page.waitForTimeout(450);

const hl = await page.evaluate(() => {
  const el = document.querySelector('.cm-tesela-priority');
  return el ? { text: el.textContent, color: getComputedStyle(el).color } : null;
});
check('p2 highlighted inline (gated on by #task)', hl && /p2/i.test(hl.text || ''), hl);
check('the inline highlight is P2 amber', hl && /232,\s*163,\s*61/.test(hl.color || ''), hl?.color);

// Enter #1 commits the #task tag (Model A) → tags:: task.
await page.keyboard.press('Enter');
await page.waitForTimeout(1700); // settle: tag-page materialize + auto-save
// Enter #2 commits the block → the Enter-lift fires (block is now a Task).
await page.keyboard.press('Enter');
await esc();
await page.waitForTimeout(1800);

const c = (await noteContent()) || '';
check('#task committed to tags::', /tags::.*task/i.test(c), c);
check('Enter-lift wrote priority:: p2 (structured)', /priority::\s*p2/i.test(c), c);
check('"p2" stripped from the prose ("ship build" kept)', /ship build\b/i.test(c) && !/ship build p2/i.test(c), c);

console.log('PAGE ERRORS:', errs.length, errs.slice(0, 3));
let pass = 0;
for (const r of results) {
  console.log(`${r.p ? 'PASS' : 'FAIL'}  ${r.n}${r.p ? '' : '   got=' + JSON.stringify(r.d)}`);
  if (r.p) pass++;
}
console.log(`\n${pass}/${results.length} passed`);
await browser.close();
process.exit(pass === results.length && errs.length === 0 ? 0 : 1);
