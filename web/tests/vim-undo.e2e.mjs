// Vim cross-block undo regression (Graphite /g) — the Loro UndoManager path
// (vim bug #12). Covers:
//   • ciw + u reverts the change
//   • Ctrl-r redoes it
//   • dw + u restores the cut word
//   • CROSS-BLOCK: edit A, edit B, u twice → A reverts while focus is on B
//
// Drives REAL keystrokes. Pauses > the doc's mergeInterval (500ms) separate
// distinct edits into distinct undo steps, mirroring human-paced editing.
//
// PREREQUISITES: same as vim-registers.e2e.mjs (build + a static-serving
// tesela-server on :7788 + `npx playwright install chromium`). Run:
//   node tests/vim-undo.e2e.mjs
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:7788/g';
const browser = await chromium.launch();
const ctx = await browser.newContext();
await ctx.addInitScript(() => {
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

const readFocused = () =>
  page.evaluate(() => {
    const el = document.querySelector('.cm-editor.cm-focused .cm-content');
    return el ? el.innerText.replace(/\n+$/, '') : null;
  });
const allTexts = () =>
  page.evaluate(() =>
    [...document.querySelectorAll('.cm-editor .cm-content')]
      .map((e) => e.innerText.replace(/\n+$/, ''))
      .filter((t) => t.length),
  );
const vim = async (seq) => {
  for (const ch of seq) {
    await page.keyboard.press(ch);
    await page.waitForTimeout(45);
  }
};
const esc = async () => { await page.keyboard.press('Escape'); await page.waitForTimeout(60); };
const STEP = 700; // > mergeInterval (500): forces a new undo step
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

// 1: ciw + u reverts to the original word.
await freshBlock('hello world');
await page.waitForTimeout(STEP);
await vim('0');
await vim('ciw');
await page.keyboard.type('goodbye', { delay: 14 });
await esc();
check('ciw replaced the word', (await readFocused()) === 'goodbye world', await readFocused());
await vim('u');
await page.waitForTimeout(200);
check('u after ciw reverts to "hello world"', (await readFocused()) === 'hello world', await readFocused());

// 2: Ctrl-r redoes it.
await vim(['Control+r']);
await page.waitForTimeout(200);
check('Ctrl-r redoes the ciw change', (await readFocused()) === 'goodbye world', await readFocused());

// 3: dw + u restores the cut word (delete is undoable).
await freshBlock('alpha beta');
await page.waitForTimeout(STEP);
await vim('0');
await vim('dw');
check('dw cut the first word', (await readFocused()) === 'beta', await readFocused());
await vim('u');
await page.waitForTimeout(200);
check('u after dw restores "alpha beta"', (await readFocused()) === 'alpha beta', await readFocused());

// 4: CROSS-BLOCK — edit A, (pause = new step), edit B, u twice; the 2nd u
//    reverts A's text while focus is on B.
await freshBlock('crossalpha');
await page.waitForTimeout(STEP);
await page.keyboard.press('o');
await page.waitForTimeout(120);
await page.keyboard.type('crossbeta', { delay: 14 });
await esc();
check('block B has crossbeta', (await readFocused()) === 'crossbeta', await readFocused());
// Undo repeatedly (a freshly-created block adds an extra container-creation
// undo step — increment-2 refinement). The invariant: A's text reverts while
// focus is on B WITHOUT ever focusing A. crossbeta (focused) reverts first.
let crossbetaGone = false;
let crossalphaGone = false;
for (let i = 0; i < 5 && !crossalphaGone; i++) {
  await vim('u');
  await page.waitForTimeout(250);
  const t = await allTexts();
  if (!t.some((x) => x.includes('crossbeta'))) crossbetaGone = true;
  if (!t.some((x) => x.includes('crossalpha'))) crossalphaGone = true;
}
check('cross-block: crossbeta reverted', crossbetaGone);
check('cross-block: crossalpha reverted while focus is on B (cross-block undo)', crossalphaGone);

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
