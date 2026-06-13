# Model Scorecard — practical evidence of which model fits which tier

> **First-class project goal** (Taylor 2026-06-13): build real proof of which models
> excel where and where they fall flat. This ledger is the source of truth for the
> tier rosters in `~/.claude/templates/tiers.md` and `AGENTS.md` "Tiered model routing".
> Markdown (not SQLite) so it is git-diffable and lives with the handoff docs.

## How to use this

- Every cheap-model batch gets reviewed on completion and logged below (append to the Batch Log).
- Score each item/model 1–5 (rubric below). Capture **what worked** and **where it fell flat**, plus **commit-reliability** (did the loop actually commit its own work?).
- The **Live Roster** is the current evidence-based routing recommendation — update it when the Batch Log moves an average meaningfully. It overrides the illustrative roster in `tiers.md`.

### Scoring rubric (1–5)

| Score | Meaning |
|---|---|
| 5 | Excellent — correct, in-scope, idiomatic, would ship as-is from a senior human |
| 4 | Good — correct + in-scope, minor polish nits (line length, a missed test update) |
| 3 | Acceptable — works and passes gates, but leaky: dead code, duplicated logic, partial spec compliance |
| 2 | Weak — needs rework; missed named deliverables or introduced scope creep |
| 1 | Poor — broken, off-scope, or failed to produce durable output |

**Reliability is tracked separately from quality.** A model can write 4/5 code and still fail
to commit it (an orchestration risk), or breach scope. Both inform ownership.

---

## Methodology & anti-bias (Taylor 2026-06-13)

**No Claude favoritism.** Kimi-k2.7, Minimax-m3, GPT-5.5, **Sonnet-4.6, and Opus-4.8** are rated on one axis, against each other. Opus is NOT assumed best; it must earn rank like everyone else.

**Two evidence grades** (the `source` field in `model-bench.jsonl`):
1. `reviewer-judgment` — a single model reviewed another's output. Fast but **bias-prone** (esp. a Claude model judging others) and it *guesses* whether a different model would've done better. All current Live-Roster numbers are this grade — treat as **preliminary**.
2. `head-to-head` — the gold standard. The SAME bounded task is attempted by every candidate model (incl. Opus + Sonnet) in **isolated git worktrees**; each output's **objective Verify** is run; then a **blind judge** (model identity hidden; prefer a mixed / non-Claude panel) ranks the diffs. This actually *measures* who did better. Merge the best passing diff (real work); log all attempts as benchmark data.

**Deterministic store:** `model-bench.jsonl` (append-only, git-tracked, SQLite-loadable: `sqlite3 :memory: -cmd '.mode json' "..."` or import) is the structured source of truth; this MD is the narrative. Conversational memory is NOT a store.

> ✅ **First head-to-head landed (2026-06-13, command-registry B1–B4, 5 models, blind panel).** It largely CONFIRMED the grade-1 roster and added the missing implementer data for Sonnet & Opus. Headlines: **gpt-5.5 WON** (most consistent, merged `4766111`); **minimax held its own** vs the Claude models; **Opus is high-ceiling-but-divisive** (shipped a real Esc-focus bug a *blind cheap-model judge caught* → not a safe default for bounded T3); **kimi produced a zero-line diff** (reliability fail #3). Rows: `model-bench.jsonl` (`source:head-to-head`).

## Live Roster (grade-1 reviewer-judgment + first grade-2 head-to-head on command-registry B1–B4)

_Last updated: 2026-06-13 (after the command-registry B1–B4 head-to-head: gpt-5.5 won; Opus & Sonnet now have implementer data)._

| Model | Dispatch ID | Tier (owns) | Ceiling | Reliability | Notes |
|---|---|---|---|---|---|
| **gpt-5.5** | `openai-codex/gpt-5.5` (pi) / `gpt-5.5` (codex) | **Senior** | M | high | **WON the 2026-06-13 head-to-head** (4.25, most consistent of 5; merged). Clean + correct + surgical; the only contestant to patch BOTH colon open-paths. Confirmed reaches T3. Also strongest **coordinator/reviewer/release** model (Codex role 4.0). |
| **minimax-m3** | `opencode-go/minimax-m3` | **Senior** | M | high | Strongest cheap performer (4.1 + 3.8); **H2H confirmed** — held its own vs Opus/Sonnet (3.5; one judge's #1). Idiomatic Rust, textbook ARIA, careful text-surgery. **3x rate-limit grace right now → preferred workhorse** for S/M across both tiers. |
| **sonnet-4.6** | (Claude / Workflow subagent) | Senior | L | high | **H2H: first implementer data — mid-pack (3.5)**; competent but cut the same conflict-detection corner as minimax. M proven (L unproven). Also a fair blind reviewer/recon subagent. |
| **opus-4.8** | (Claude / main loop) | **Lead** | XL | high | **H2H: high-ceiling but DIVISIVE (4.0)** — two judges rated it the cleanest design (only one with cross-surface conflict detection), two dinged a real Esc-focus bug + scope creep. **NOT a safe default for bounded T3.** Reserve for Lead/XL (sync spine, Loro, FFI, pairing, architecture) + review. |
| **kimi-k2.7-code** | `opencode-go/kimi-k2.7-code` | **Junior** ⬇ | M | **very low** | **Reliability fail #3: a zero-line diff in the head-to-head** (gates pass vacuously). Prior: spec misses + dead code (3.0) + a ralph no-commit incident. AVOID for durable work; Junior-S mechanical only, with an enforced commit checkpoint + senior review. |
| **qwen-3.7-max** | `opencode-go/qwen3.7-max` | Senior (unproven) | M (assumed) | unknown | Not yet exercised on this repo. 1M ctx / 65K out. Trial on a bounded M item to place it. |

**Routing implication:** for fan-out, **gpt-5.5 and minimax-m3 are co-leads** — gpt-5.5 won the head-to-head; minimax has 3× rate-limit grace, so prefer minimax while the grace applies, else gpt-5.5. Use **kimi** sparingly (Junior-S mechanical only, enforced commit checkpoint — 3 durable-output failures). Reserve **Opus** for Lead/XL (sync spine, Loro, FFI, pairing, architecture) + review; it is neither cost-effective nor even the safest choice for bounded T3.

---

## Batch Log (append-only)

### 2026-06-13 — HEAD-TO-HEAD (grade-2) — command-registry B1–B4 — 5 models, blind panel

- **The gold-standard run.** Same bounded task (4 command-registry gaps, `.bench/task.md`), 5 models in isolated worktrees, objective Verify, then a **blind 4-judge panel** (2× Sonnet + gpt-5.5 + minimax, identities hidden as cand A–E) + a de-anonymized adversarial pre-merge check. Fable judge unavailable. Raw votes: `.bench/blind/`.
- **Result (avg / Borda / final rank): gpt-5.5 4.25/16/🥇 · Opus 4.0/16/2 · minimax 3.5/12/3 · Sonnet 3.5/12/4 · kimi 1.0/4/5.**
- **gpt-5.5 WON → merged (`4766111`).** All 4 reqs met by every judge; most consistent; both colon open-paths patched; no scope creep. Nit: slash chords injected as synthetic command stubs vs a dedicated map → follow-up Junior-S cleanup.
- **Opus tied on Borda but LOST on correctness:** never patches the GrRail rail-click open path, so Esc drops focus after a rail-initiated colon open (real partial req-3 miss) — caught by a *blind cheap-model judge (gpt-5.5)*. High ceiling (two judges: cleanest design, only cross-surface slash↔leader conflict detection) but divisive. **Lesson: Opus is not a safe default for bounded T3; reserve for genuinely hard work.**
- **minimax** held its own vs the Claude models (one judge's #1); req-1 partial (findConflicts doesn't consume slash chords) + dead union member. Senior T2/T3 confirmed.
- **Sonnet** (first implementer data this cycle): mid-pack (3.5), competent, cut the same conflict-detection corner as minimax. T2.
- **kimi: reliability failure #3** — a *zero-line diff* in a fully-specced isolated worktree (gates pass vacuously on baseline). After the ralph no-commit incident, this is a pattern, not an incident. Avoid for durable work.
- **Anti-bias validation:** judging was blind; the **minimax judge ranked its own (blind) diff 4th** → no self-favoritism. A cheap open model topping Opus is therefore a credible signal, not Claude-deprecation noise.

### 2026-06-13 — `minimax-m3` — A1–A4 mechanical Rust batch (Junior loop, ralph)

- **Avg: 3.8/5.** Ceiling confirmed M. Reliability: high (all four committed cleanly).
- A1 clippy (`9c1e2d8`) — **good**: all ~14 fixes mechanically correct; nits: doc-comment reformatting unrelated to clippy (scope creep) + a `pub` trait method rename it should have flagged.
- A2 MCP unwrap→expect (`5396822`) — **acceptable**: invariant claim correct; 182-char copy-pasted `.expect()` message at all 3 sites.
- A3 Logseq unwrap→expect (`9240a71`) — **excellent**: caught a wrong-file spec pointer, navigated to the real module, verified both regexes, documented the discrepancy. Best in batch.
- A4 backup constants (`dcce557`) — **good**: correct constants + re-exports, but left literal `7/4/6` in `backup_scheduler.rs` tests/doc after the commit msg claimed cross-crate consistency (self-imposed gap). → follow-up Junior-S backlog item.

### 2026-06-13 — `kimi-k2.7-code` — B1–B3 command-registry foundation (Senior loop, ralph)

- **Avg: 3.0/5** (B2/B3 only; B1 was rescued by Pi). Reliability: **LOW** — see below. → demoted to Junior.
- B1 unified registry spine (`6f3f90f`) — **excellent**, but authored by **Pi**, not kimi (kimi's loop advanced iterations without committing). Not credited to kimi.
- B2 keymap introspection (`012a556`) — **good but spec-non-compliant**: didn't index `BUILTIN_SLASH_CHORDS` (spec named the constant explicitly); `formatKeymap` omits commands without shortcuts/chords.
- B3 context-aware dispatch (`6b1cb33`) — **acceptable, leaky**: left `command-context.svelte.ts` as dead/orphaned code; duplicated the when-predicate loop in `leader-tree` instead of delegating to `commandRegistry.available()`; left `ColonCommandLine` on `.all()` (no context filter).
- **Reliability incident:** kimi's ralph loop appeared to make progress but never committed B1 — a "busy but no durable output" failure mode. For multi-file senior items, kimi needs enforced commit checkpoints; do not trust it to close its own loop. This is precisely the hand-off-thrash risk to design against.

### 2026-06-12 — codex-pi batch (mixed) — 12 items, Codex-coordinated

- **minimax-m3 (items 1–9): 4.1/5 → Senior, ceiling M.** Items 1/2/5/6 genuinely excellent (textbook ARIA combobox/listbox wiring; correct `(modifiers, code)` match-arm ordering in the TUI; a `markdown.rs` frontmatter helper handling 6 YAML tag forms byte-preserving; best module docs in the batch). Item 3 had 2 first-pass test-data bugs (backlink raw value), corrected on a second pass. Item 4 VoiceOver verify blocked by headless idb (honest limitation). Correctly respected off-limits zones (sync hot path, FFI).
- **gpt-5.5 (items 10–12): 3.8/5 → Senior, ceiling M.** Item 12 (per-line property regex hardening) was the cleanest diff in the batch. Item 10 (fenced code render) conceptually correct but rough: unclosed-fence fallthrough treats the rest of the doc as code; double-calls `findCodeFenceSpans`. Item 11 test relies on an untested line-id→bid resolution assumption. No scope creep; correctly declined an out-of-scope pre-existing bug.
- **Codex / gpt-5.5 (coordinator): 4.0/5.** Correctly refused to implement feature items; ran thorough Junior/Senior validation passes; executed desktop notarization + iOS TestFlight release. One shakiness: a browser QA pass hit a wrong API base path (405s) and needed a retry.
- **Batch-level:** clean — all items landed with passing tests, correct scope, no off-limits edits, honest limitation reporting. Recurring debt: `cargo fmt --all` drift consistently noted as out-of-scope rather than fixed.
