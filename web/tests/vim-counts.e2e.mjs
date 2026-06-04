// Vim count regression (Graphite /g) — `Nj` / `Nk` cross-block navigation
// (vim bug #2). Counts read off cm-vim's actionArgs.repeat and thread through
// the outliner's handleNavigate(dir, count).
//
// Uses the /tmp/mdqa daily note's first four (deterministic, non-empty) blocks:
//   0 bold…  1 link…  2 image (![…])  3 hr (---)
//
// PREREQUISITES: same as vim-registers.e2e.mjs. Run: node tests/vim-counts.e2e.mjs
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:7788/g';
const browser = await chromium.launch();
const ctx = await browser.newContext();
await ctx.addInitScript(() => { window.__TESELA_API_BASE__ = ''; });
const page = await ctx.newPage();
const errs = [];
page.on('pageerror', (e) => errs.push('PAGEERR ' + e.message));

await page.goto(URL, { waitUntil: 'domcontentloaded' });
await page.waitForSelector('.cm-editor', { timeout: 15000 });
await page.waitForTimeout(1500);

const results = [];
const check = (name, pass, detail) => results.push({ name, pass: !!pass, detail });

const focused = () =>
  page.evaluate(() => {
    const el = document.querySelector('.cm-editor.cm-focused');
    return {
      txt: el?.querySelector('.cm-content')?.innerText.replace(/\n+$/, '') ?? null,
      normal: !!el?.querySelector('.cm-fat-cursor'),
    };
  });
const press = async (k) => { await page.keyboard.press(k); await page.waitForTimeout(80); };

await page.click('.cm-content');
await page.waitForTimeout(250);
await press('Escape'); // normal mode, block 0

// 2j → block 2 (the image block), still NORMAL mode.
await press('2'); await press('j');
let s = await focused();
check('2j jumps two blocks down', s.txt?.includes('placeholder image'), JSON.stringify(s));
check('2j stays in normal mode', s.normal, JSON.stringify(s));

// 2k → back to block 0 (bold).
await press('2'); await press('k');
s = await focused();
check('2k jumps two blocks up to the first block', s.txt?.includes('**bold**'), JSON.stringify(s));

// 3j → block 3 (the horizontal rule "---").
await press('3'); await press('j');
s = await focused();
check('3j jumps three blocks down', s.txt?.trim() === '---', JSON.stringify(s));

console.log('=== PAGE ERRORS (' + errs.length + ') ===');
for (const e of errs) console.log(e);
console.log('=== RESULTS ===');
let passN = 0;
for (const r of results) {
  console.log(`${r.pass ? 'PASS' : 'FAIL'}  ${r.name}${r.pass ? '' : '   got=' + r.detail}`);
  if (r.pass) passN++;
}
console.log(`\n${passN}/${results.length} passed`);
await browser.close();
process.exit(passN === results.length ? 0 : 1);
