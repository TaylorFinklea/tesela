// Generates the evergreen harness-deck model-eval scorecard for the Tesela fleet.
// Re-run any time model-bench.jsonl changes; it overwrites the report in place.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const BENCH = ".docs/ai/model-bench.jsonl";
const rows = fs.readFileSync(BENCH, "utf8").trim().split("\n").map((l) => JSON.parse(l));

// Normalize model labels that drifted across sessions.
const norm = (m) => {
  const s = (m || "?").toLowerCase();
  if (s.includes("sonnet")) return "Sonnet 4.6";
  if (s.includes("gpt-5.5")) return "GPT-5.5";
  if (s.includes("minimax")) return "minimax-m3";
  if (s.includes("kimi")) return "kimi-k2.7-code";
  if (s.includes("opus")) return "Opus 4.8 (Lead)";
  return m;
};

const by = {};
for (const r of rows) (by[norm(r.model)] ??= []).push(r);

const agg = Object.entries(by).map(([model, rs]) => {
  const scored = rs.filter((r) => typeof r.score_1_5 === "number");
  const avg = scored.length ? scored.reduce((a, r) => a + r.score_1_5, 0) / scored.length : 0;
  return {
    model,
    items: rs.length,
    avg: +avg.toFixed(2),
    fives: rs.filter((r) => r.score_1_5 === 5).length,
    passed: rs.filter((r) => r.verify_passed).length,
    min: scored.length ? Math.min(...scored.map((r) => r.score_1_5)) : 0,
    impl: rs.filter((r) => r.role === "implementer").length,
  };
}).sort((a, b) => b.avg - a.avg || b.items - a.items);

// Verdict per model (hand-tuned from the evidence).
const verdict = {
  "GPT-5.5": ["workhorse — RATE-LIMITED", "--tn-orange"],
  "Sonnet 4.6": ["new lead — strongest available", "--tn-green"],
  "qwen3.7-max": ["strong debut (opencode-go OK)", "--tn-cyan"],
  "minimax-m3": ["solid output, flaky under load", "--tn-yellow"],
  "kimi-k2.7-code": ["capable via openrouter only", "--tn-yellow"],
  "Opus 4.8 (Lead)": ["Lead / reviewer / scorer", "--tn-blue"],
};

const maxItems = Math.max(...agg.map((a) => a.items));
const rowHtml = agg.map((a) => {
  const [vtxt, vcol] = verdict[a.model] ?? ["—", "--tn-comment"];
  const barW = Math.round((a.avg / 5) * 100);
  const itemBar = Math.round((a.items / maxItems) * 100);
  return `<tr>
    <td style="font-weight:600;white-space:nowrap">${a.model}</td>
    <td style="text-align:right;color:var(--tn-comment)">${a.items}<span style="display:inline-block;width:36px;height:5px;margin-left:6px;background:var(--tn-rule);border-radius:3px;vertical-align:middle"><span style="display:block;height:100%;width:${itemBar}%;background:var(--tn-blue);border-radius:3px"></span></span></td>
    <td style="text-align:right;font-variant-numeric:tabular-nums;font-weight:600">${a.avg.toFixed(2)}</td>
    <td><span style="display:inline-block;width:90px;height:7px;background:var(--tn-rule);border-radius:4px"><span style="display:block;height:100%;width:${barW}%;background:${vcol === "--tn-comment" ? "var(--tn-blue)" : `var(${vcol})`};border-radius:4px"></span></span></td>
    <td style="text-align:center;color:var(--tn-green)">${a.fives}</td>
    <td style="text-align:center;color:${a.min <= 2 ? "var(--tn-red)" : "var(--tn-comment)"}">${a.min || "—"}</td>
    <td style="color:var(${vcol});font-size:12px">${vtxt}</td>
  </tr>`;
}).join("\n");

const scorecardHtml = `<style>
  .sc{width:100%;border-collapse:collapse;font-size:13px}
  .sc th{text-align:left;color:var(--tn-comment);font-weight:600;font-size:11px;text-transform:uppercase;letter-spacing:.6px;padding:0 10px 8px;border-bottom:1px solid var(--tn-rule)}
  .sc td{padding:9px 10px;border-bottom:1px solid var(--tn-rule)}
  .sc tr:last-child td{border-bottom:none}
  .sc th.r,.sc td.r{text-align:right}
</style>
<table class="sc">
  <thead><tr>
    <th>Model</th><th class="r">Items</th><th class="r">Avg /5</th><th>Quality</th>
    <th style="text-align:center">5/5</th><th style="text-align:center">Min</th><th>Verdict</th>
  </tr></thead>
  <tbody>${rowHtml}</tbody>
</table>`;

// --- Capability-by-work-type matrix -----------------------------------------
// Opus's HOLISTIC 1–5 ratings per kind of work, synthesized from reviewing
// EVERY fleet batch — NOT a mechanical average of the per-task scores above.
// Reads as "how good is this model AT this kind of work", independent of which
// tasks it happened to draw. null = no valid (non-provider-broken) sample yet.
// low:true = <5 items, treat as provisional. UPDATE THIS as new batches land.
const CAPABILITY = {
  "GPT-5.5": {
    arch: 5, impl: 5, debug: 5, refac: 5, instr: 5, rel: 5,
    note: "Gold standard across Rust · TS · Swift · bash — 11× 5/5 incl. the hardest items unaided. Only weakness is availability (now rate-limited), not output.",
  },
  "Sonnet 4.6": {
    arch: 5, impl: 5, debug: 5, refac: 5, instr: 5, rel: 4,
    note: "Cleanest architecture in head-to-heads: won the ED1 3-way (self-contained StateField) and deleted the applySlash switch (north-star #1). Web/TS only so far — Rust/Swift unproven. KB4 shipped 2 typecheck-passing runtime gaps → self-verification 4.",
  },
  "minimax-m3": {
    arch: 3, impl: 4, debug: 4, refac: 4, instr: 4, rel: 2,
    note: "Broadest language range (Rust + Swift + TS). Solid 4.5–5 on S/mechanical, but couldn't crack the hard ED1 render (3.0). Reliability is the headline flaw: 2064 high-load errors + a zero-diff non-attempt (1.0) + an errored-after-completion run.",
  },
  "qwen3.7-max": {
    arch: 4, impl: 4, debug: 4, refac: null, instr: 4, rel: 4, low: true,
    note: "Strong debut but only 2 items (low confidence). Correct StateField architecture, slightly more coupled than Sonnet's. Runs clean via opencode-go (the same provider that broke kimi — so the kimi failure was model/route-specific).",
  },
  "kimi-k2.7-code": {
    arch: 3, impl: 2, debug: 2, refac: null, instr: 3, rel: 1, low: true,
    note: "opencode-go = ZERO edits (PROVIDER fault, not the model → both refactor/migration attempts were void, hence —). Via openrouter it's capable: got the StateField architecture right, but reintroduced a runtime trap (dispatch inside a ViewPlugin update()). Route via openrouter only.",
  },
};

const capDims = [
  ["arch", "Arch"], ["impl", "Impl"], ["debug", "Debug"],
  ["refac", "Refactor"], ["instr", "Instr"], ["rel", "Reliab"],
];
const capColor = (v) =>
  v == null ? "var(--tn-comment)"
  : v >= 4.5 ? "var(--tn-green)"
  : v >= 3.5 ? "var(--tn-cyan)"
  : v >= 2.5 ? "var(--tn-yellow)"
  : "var(--tn-red)";
const capCell = (v) =>
  `<td style="text-align:center;font-weight:700;font-variant-numeric:tabular-nums;color:${capColor(v)}">${v == null ? "—" : v}</td>`;

// Keep capability rows in the same order as the main aggregate (by avg desc).
const capRows = agg.map((a) => a.model).filter((m) => CAPABILITY[m]).map((m) => {
  const c = CAPABILITY[m];
  const tag = c.low ? `<span style="color:var(--tn-comment);font-size:11px" title="low sample (&lt;5 items)"> °</span>` : "";
  return `<tr>
    <td style="font-weight:600;white-space:nowrap">${m}${tag}</td>
    ${capDims.map(([k]) => capCell(c[k])).join("")}
    <td style="font-size:11px;color:var(--tn-comment);line-height:1.45">${c.note}</td>
  </tr>`;
}).join("\n");

const capabilityHtml = `<style>
  .cap{width:100%;border-collapse:collapse;font-size:13px}
  .cap th{text-align:center;color:var(--tn-comment);font-weight:600;font-size:11px;text-transform:uppercase;letter-spacing:.5px;padding:0 8px 8px;border-bottom:1px solid var(--tn-rule)}
  .cap th:first-child,.cap th:last-child{text-align:left}
  .cap td{padding:9px 8px;border-bottom:1px solid var(--tn-rule);vertical-align:top}
  .cap tr:last-child td{border-bottom:none}
  .cap .leg{font-size:11px;color:var(--tn-comment);padding-top:9px}
</style>
<table class="cap">
  <thead><tr>
    <th>Model</th>${capDims.map(([, h]) => `<th>${h}</th>`).join("")}<th>Notes</th>
  </tr></thead>
  <tbody>${capRows}</tbody>
</table>
<div class="leg">Cell colors: <span style="color:var(--tn-green);font-weight:700">≥4.5</span> · <span style="color:var(--tn-cyan);font-weight:700">3.5–4.4</span> · <span style="color:var(--tn-yellow);font-weight:700">2.5–3.4</span> · <span style="color:var(--tn-red);font-weight:700">&lt;2.5</span> &nbsp;|&nbsp; <b>—</b> no valid sample · <b>°</b> low sample (&lt;5 items)</div>`;

const total = rows.length;
const implTotal = rows.filter((r) => r.role === "implementer").length;

const report = {
  schema: "harness-deck/report@1",
  id: "model-eval-scorecard",
  project: "tesela",
  harness: "claude-code",
  kind: "report",
  title: "Model Eval Scorecard — Tesela Fleet",
  status: "done",
  created: "2026-06-13T00:00:00Z",
  scope: "Which cheap/fast models excel where, and where they fall flat — git-tracked, every fleet batch Opus-reviewed + scored 1–5.",
  blocks: [
    {
      type: "prose",
      markdown: `**Evergreen.** Updated as new fleet batches land (source of truth: \`.docs/ai/model-bench.jsonl\`, **${total} rows**, ${implTotal} implementer items). Each item: a real task dispatched to a model in an isolated git worktree, self-verified (build/test), then **Opus-reviewed, merged, and scored 1–5** against the live codebase. Not blind (the original head-to-head that crowned GPT-5.5 was blind; fleet items are reviewed merges).`,
    },
    { type: "html", html: scorecardHtml },
    {
      type: "prose",
      markdown: `### Capability by work-type\n\n**Opus's holistic 1–5 ratings** — synthesized from reviewing *every* batch, **not** a mechanical average of the per-task scores above. Each cell answers "how good is this model **at this kind of work**," independent of which tasks it happened to draw. The dimensions: **Arch** (design/structural calls) · **Impl** (greenfield from a spec) · **Debug** (diagnose + fix) · **Refactor** (gut-and-rewire, byte-identical migration) · **Instr** (spec adherence, scope discipline) · **Reliab** (actually finishes — no load-fails / zero-diffs / runtime traps). \`—\` = no valid sample yet; **°** = low sample (<5 items), provisional.`,
    },
    { type: "html", html: capabilityHtml },
    {
      type: "recommendations",
      items: [
        { id: "gpt55", markdown: "**GPT-5.5 hit rate limits (2026-06-13).** Was the workhorse (4.85 avg, 11× 5/5 across Rust/TS/Swift/bash). Replaced by Sonnet 4.6 for must-land work until limits reset." },
        { id: "sonnet", markdown: "**Sonnet 4.6 = the new default for hard/must-land items.** Two 5/5 on the hardest items: B-impl-4a (6-verb slash migration) and B-impl-4b (deleted the legacy `applySlash` switch — completed north-star #1, beat kimi in a head-to-head). Byte-identical mappings, honest test updates, no overreach. Clean GPT-5.5 stand-in via `pi --model openrouter/anthropic/claude-sonnet-4.6`." },
        { id: "minimax", markdown: "**minimax-m3 = S/mechanical only.** Output quality is 4.5–5 when it lands, but it hits 2064 high-load errors under volume (2 failures: a zero-line diff + an errored-after-completion run). Avoid for must-land items." },
        { id: "qwen", markdown: "**Qwen 3.7 Max — strong debut (4.5).** First fleet item (ED1-fix, GFM-table StateField render, 3-way vs Sonnet+kimi): correct StateField architecture, wired into both editor sites, safe rAF-deferred focus dispatch — just slightly more coupled than Sonnet's self-contained tracker. Runs fine via `opencode-go/qwen3.7-max` (the same provider that BROKE kimi — so the kimi failure was kimi-specific, not provider-wide)." },
        { id: "kimi", markdown: "**kimi-k2.7-code — root cause was the `opencode-go` provider, NOT the model.** All prior zero-diffs came from `opencode-go/kimi-k2.7-code` (tool-calls never reached the model). Switched to **`openrouter/moonshotai/kimi-k2.7-code`** → it produced a real 528-line diff and got the StateField architecture right. (It did reintroduce a runtime trap — dispatching inside a ViewPlugin `update()` — so 3.5, not 5.) **Route kimi via openrouter; never opencode-go.** Matches the earlier litellm-proxy issue." },
        { id: "opus", markdown: "**Opus 4.8 = Lead / reviewer / scorer.** Writes specs, decomposes for lower tiers, and is the merge gate — caught a runtime-broken table render, a false test-flake, a zero-diff non-attempt, and stale-doc reverts in fleet diffs." },
      ],
    },
    {
      type: "callout",
      level: "info",
      markdown: "**Method note:** the **top table** (Items / Avg / 5-5 / Min) is mechanical — regenerated from the bench ledger, never drifts. The **capability matrix** is hand-tuned by Opus from reviewing every batch (it's a judgment, not an average) and lives in the generator's `CAPABILITY` map — update it as new batches land. Model labels are normalized (e.g. `sonnet-4.6`/`claude-sonnet-4.6` → Sonnet 4.6); \"Min\" = lowest score seen (load-failures show as 1).",
    },
  ],
  meta: [
    { key: "generator", value: ".bench/gen-scorecard-report.mjs" },
    { key: "bench_rows", value: String(total) },
  ],
};

const dir = path.join(os.homedir(), ".harness", "reports", "tesela", "model-eval-scorecard");
fs.mkdirSync(dir, { recursive: true });
const file = path.join(dir, "report.json");
fs.writeFileSync(file, JSON.stringify(report, null, 2));
console.log("Wrote", file);
console.log("Models:", agg.map((a) => `${a.model}=${a.avg}(${a.items})`).join("  "));
