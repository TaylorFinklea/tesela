// Vim register-cluster regression (Graphite /g, per-block cm-vim).
//
// Covers the bugs fixed in BlockEditor.svelte's custom delete/yank/paste:
//   • dw / d$ cut text into the register so `p` pastes it back   (bug #5)
//   • yiw / yw copy to the register AND the system clipboard      (bug #6)
//   • named registers: "ayiw … "ap                                (bug #6)
//   • block yy → p still creates a whole new block (no regression)
//   • cross-block paste: yank in block A, `p` in block B
//
// Drives REAL keystrokes through cm-vim, so it exercises the whole chain
// (key → operator → register controller → paste), not the JS API directly.
//
// PREREQUISITES (no Playwright test-runner in this repo — run by hand):
//   1. pnpm build                                  # build the /g bundle
//   2. start a server that serves it + the API on one origin, e.g.:
//        TESELA_SERVER_BIND=127.0.0.1:7788 \
//        TESELA_STATIC_DIR="$PWD/build" \
//        TESELA_DEFAULT_MOSAIC=/tmp/mdqa \
//        TESELA_DISABLE_MDNS=1 TESELA_DISABLE_RELAY=1 TESELA_DISABLE_PEER_SYNC=1 \
//        ../target/debug/tesela-server &
//   3. npx playwright install chromium             # once
//   4. node tests/vim-registers.e2e.mjs            # REPRO_URL overrides the URL
//
// Exit code 0 = all pass.
import { chromium } from '@playwright/test';

const URL = process.env.REPRO_URL || 'http://127.0.0.1:7788/g';
const browser = await chromium.launch();
const ctx = await browser.newContext();
await ctx.addInitScript(() => {
  // Mirror the Tauri shell: same-origin API. Spy on clipboard writes so the
  // "yank → system clipboard" assertion doesn't depend on OS clipboard perms.
  window.__TESELA_API_BASE__ = '';
  window.__clip = [];
  try {
    if (navigator.clipboard) {
      navigator.clipboard.writeText = (t) => {
        window.__clip.push(String(t));
        return Promise.resolve();
      };
    }
  } catch (e) {}
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
const clip = () => page.evaluate(() => window.__clip.slice());
const blockCount = () => page.evaluate(() => document.querySelectorAll('.cm-editor').length);

const vim = async (seq) => {
  for (const ch of seq) {
    await page.keyboard.press(ch);
    await page.waitForTimeout(45);
  }
};
const esc = async () => {
  await page.keyboard.press('Escape');
  await page.waitForTimeout(60);
};
const freshBlock = async (text) => {
  await esc();
  await page.keyboard.press('o'); // newBlockBelow → insert
  await page.waitForTimeout(120);
  await page.keyboard.type(text, { delay: 14 });
  await page.waitForTimeout(120);
  await esc();
};

await page.click('.cm-content');
await page.waitForTimeout(250);

// 1: yiw → system clipboard, then p pastes inline.
await freshBlock('alpha beta gamma');
await vim('0');
await vim('yiw');
check('yiw → clipboard gets "alpha"', (await clip()).includes('alpha'));
await vim('$');
await vim('p');
check('yiw then p pastes inline', (await readFocused()) === 'alpha beta gammaalpha', await readFocused());

// 2: dw cuts a word into the register, p pastes it back.
await freshBlock('one two three');
await vim('0');
await vim('dw');
check('dw deletes first word', (await readFocused()) === 'two three', await readFocused());
await vim('$');
await vim('p');
check('dw then p pastes the cut word', (await readFocused()) === 'two threeone ', JSON.stringify(await readFocused()));

// 3: named register "ayiw / "ap.
await freshBlock('red green blue');
await vim('0');
await vim('"ayiw');
await vim('$');
await vim('"ap');
check('named register "ayiw then "ap', (await readFocused()) === 'red green bluered', await readFocused());

// 4: block yy then p still creates a new block (regression guard).
await freshBlock('blockone');
const cntBefore = await blockCount();
await vim('yy');
await vim('p');
await page.waitForTimeout(400);
const cntAfter = await blockCount();
check('yy then p adds a block (block paste preserved)', cntAfter === cntBefore + 1, `${cntBefore}->${cntAfter}`);

// 5: cross-block charwise paste (yank in A, p in B).
await freshBlock('sourceword target');
await vim('0');
await vim('yiw');
await page.keyboard.press('o');
await page.waitForTimeout(120);
await page.keyboard.type('dest ', { delay: 14 });
await esc();
await vim('$');
await vim('p');
check('cross-block charwise paste', (await readFocused()) === 'dest sourceword', JSON.stringify(await readFocused()));

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
