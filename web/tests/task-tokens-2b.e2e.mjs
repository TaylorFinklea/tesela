// Model B Part 2b regression (Graphite /g): per-tag-gated inline NLP.
//   • ⌘↵ make-task = "tag it AND parse it": lifts priority + multi-word dates.
//   • dates: bare → scheduled, `due`/`deadline` keyword → deadline (multi-word
//     like "next tuesday" works — the reason markers were rejected).
//   • GATING: detection runs ONLY on blocks whose DIRECT tags include a
//     detect-enabled tag (default #Task). A non-Task block (or a #journal block)
//     never detects — this is the inheritance fix (a child inheriting #Task but
//     directly tagged #journal stays prose).
//
// PREREQS: vite dev on :5173 + a fresh tesela-server on :7474 + playwright.
// Run: REPRO_URL=http://127.0.0.1:5173/g node tests/task-tokens-2b.e2e.mjs
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
const freshBlock = async (text) => {
  await esc();
  await page.keyboard.press('o');
  await page.waitForTimeout(150);
  await page.keyboard.type(text, { delay: 16 });
  await page.waitForTimeout(1600); // let the block auto-save (bid resolves before property ops)
};
const ISO = /\d{4}-\d{2}-\d{2}/;

await page.click('.cm-content');
await page.waitForTimeout(250);

// ── 1) ⌘↵ make-task: multi-word date + priority ─────────────────────────────
await freshBlock('call mom next tuesday p1');
await page.keyboard.press('Meta+Enter'); // make-task = tag-it-and-parse-it
await esc();
await page.waitForTimeout(1800);
{
  const c = (await noteContent()) || '';
  // isolate the "call mom" block (the daily can hold other blocks)
  check('make-task tagged it #Task', /tags::.*task/i.test(c), c);
  check('priority:: p1 lifted', /priority::\s*p1/i.test(c), c);
  check('scheduled:: <date> lifted (next tuesday → ISO)', new RegExp('scheduled::\\s*\\[?\\[?' + ISO.source).test(c), c);
  check('prose stripped to "call mom" (no "next tuesday"/"p1" inline)', /call mom\b/i.test(c) && !/next tuesday/i.test(c) && !/call mom.*\bp1\b/i.test(c), c);
}

// ── 2) ⌘↵ make-task: `due` keyword → deadline ───────────────────────────────
await freshBlock('submit report due friday');
await page.keyboard.press('Meta+Enter');
await esc();
await page.waitForTimeout(1800);
{
  const c = (await noteContent()) || '';
  check('"due friday" → deadline:: <date>', new RegExp('deadline::\\s*\\[?\\[?' + ISO.source).test(c), c);
  check('"submit report" prose kept, "due friday" stripped', /submit report\b/i.test(c) && !/due friday/i.test(c), c);
}

// ── 3) GATING: a NON-task block does NOT lift priority (false-positive fix) ──
await freshBlock('review p1 status');
await page.keyboard.press('Enter'); // plain Enter, NOT make-task
await esc();
await page.waitForTimeout(1200);
{
  const c = (await noteContent()) || '';
  check('non-task block: p1 NOT lifted (stays inline)', /review p1 status/i.test(c), c);
}

// ── 4) GATING: a #journal block does NOT detect a date (the inheritance case) ─
await esc();
await page.keyboard.press('o');
await page.waitForTimeout(150);
await page.keyboard.type('#journal', { delay: 16 });
await page.waitForTimeout(400);
await page.keyboard.press('Escape'); // leave #journal inline (don't commit as a chip)
await page.waitForTimeout(120);
await page.keyboard.type(' recalled yesterday', { delay: 16 });
await page.waitForTimeout(1600);
await page.keyboard.press('Enter');
await esc();
await page.waitForTimeout(1200);
{
  const c = (await noteContent()) || '';
  check('#journal block: "yesterday" NOT detected (no new scheduled on it)', /recalled yesterday/i.test(c), c);
}

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
