# Current State

## Branch
- `main` (ahead of origin; unpushed — 43+ commits incl. `4766111` cmd-registry B1–B4, waves 1–4 merges, L2 spec). NOT pushed.
- Opus = orchestrator/Lead. Fleet = gpt-5.5 + minimax via pi (Bash); Claude subagents via Workflow. Evidence: `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Waves 1–3 DONE (10 items). ED1 held — Lead `ED1-fix` (ViewPlugin can't line-break-replace; salvage in `.bench/wave3/logs/ed1.diff.patch`).
- [x] **Wave 4 DONE — 5/6 merged+scored:** DSK5(minimax 5 `9aec7bc9`), DSK6(gpt-5.5 5 `c4820230`), DSK7(gpt-5.5 5 `58e974bb`), PROP2(gpt-5.5 5 `9f14b254`), PROP4(minimax 4.5 `b1090353`). **PROP5 non-attempt** (minimax rate-limited 2064 → 0 lines) → re-dispatched wave5 on gpt-5.5.
- [x] **L2 slash-registry spec DONE** — `phases/2026-06-13-slash-registry-spec.md` (verb-only SlashContext, additive `run(arg?,ctx?)`, surface-gating, gut-don't-delete+grep gate). → B-impl-1..4 in backlog (unblocked, sequential senior chain). 4 keyboard-first open Qs for Taylor in spec (non-blocking).
- [x] **Wave 5 DONE — 5/5 merged+scored:** IOS1(gpt-5.5 5 `7e5bf379`), IOS2(minimax 5 `312d7e8f`), IOS3(minimax 5 `ba413d26`), ED2(gpt-5.5 5 `892e0ccc`) — combined incremental xcodebuild GREEN; all compiled first-try unaided. **PROP5**(gpt-5.5 5 `45d834cd`) — re-dispatch succeeded where minimax rate-limited; Opus flipped default ON→OFF (opt-in) + excluded its stale .docs edits.
- [x] **B-impl-1 DONE** (wave6, gpt-5.5 5/5) — registry types widened (editor?/surface/slashKey, `run(arg?,ctx?)`), ctx threaded through colon/palette/leader, `slash-context.ts` SlashContext type. Purely additive.
- [x] **B-impl-2+3 DONE** (wave7, gpt-5.5 5/5) — `buildSlashContext()` producer + heading/date verbs dispatch through the unified registry (slash now shares the registry with `:`/⌘K/leader — north-star milestone). Opus-reviewed all 7 spec hard constraints + proved heading caret byte-identical. 11 verbs still on the legacy switch.
- [x] **Runtime self-QA PASSED** (2026-06-13, real browser /g) — slash menu heading+date first (no dupes), 11 legacy intact; /heading→`# `; /date→picker; editor.date filtered from ⌘K; zero console errors. North-star slash-via-registry CONFIRMED working.
- [x] **B-impl-4a DONE** (wave8, **Sonnet 4.6** 5/5) — 6 flat verbs (task/tag/link/query/collection/template) → registry; cases gutted; mappings byte-identical to legacy; runtime QA PASSED (/task→Task chip via addTag, no console errors). Switch + widget/property/`/p`/`/s` intact. **Sonnet 4.6 is a clean gpt-5.5 replacement.**
- [x] **B-impl-4b DONE — HEAD-TO-HEAD** (wave9): **Sonnet 4.6 WON 5/5** (deleted the applySlash switch cleanly, runtime-QA'd) vs **kimi-k2.7-code = zero-diff non-attempt** (4th time). **🎉 NORTH-STAR #1 COMPLETE: slash + `:` + ⌘K + leader all on one `commandRegistry`.** B-impl chain (1→4b) fully merged.
- **🔑 KIMI ROOT CAUSE FOUND (2026-06-13):** the zero-diffs were the **`opencode-go` PROVIDER/proxy**, NOT the model or pi. Proof: `openrouter/moonshotai/kimi-k2.7-code` (pi) **edits files fine**; `opencode-go/kimi-k2.7-code` produces nothing. So **route kimi via `openrouter/moonshotai/kimi-k2.7-code`** (no opencode CLI needed). ⚠ The `opencode-go` provider may break OTHER models' tool-calling too (watch qwen3.7-max — also opencode-go). The `opencode run` CLI path is blocked by the harness classifier (`--dangerously-skip-permissions`) — not needed now.
- **Qwen 3.7 Max IN ROTATION** (Taylor's call): `opencode-go/qwen3.7-max` (1M ctx). If it zero-diffs via opencode-go, fall back to an openrouter qwen variant.
- [x] **Wave 10 DONE — ED1-fix 3-way** (GFM table via StateField): **Sonnet 4.6 WON 5/5** (runtime-QA'd: unfocused table renders, zero console errors), Qwen 3.7 Max 4.5 (correct, more coupled), kimi/openrouter 3.5 (correct StateField but dispatch-in-update runtime trap). ED1-fix MERGED — the held item is closed. All 3 used a StateField (prompt guidance landed).
- **Provider learnings:** qwen3.7-max works via `opencode-go`; **kimi MUST use openrouter** (`opencode-go/kimi` = zero edits). Scorecard live: gpt-5.5 4.79 · Sonnet 4.63(5) · qwen 4.5(1) · minimax 4.14 · kimi 2.13(4).
- [x] **L1 sync CORE DONE** (spec `phases/2026-06-13-l1-sync-spec.md`). **Scope inverted by tree-verified scoping: crypto spine + seq-fix + A5 cursor hardening were ALREADY shipped** — L1 = a plumbing bug, not a re-architecture. Shipped: **PV** (`693dd9a0` joiner persists relay_url → joins the spine, the real fix), **CT** (`a8c46e36` seq-black-hole fence), **GKS** (`8582be46` GroupKeyStore seam). Decisions on defaults: defer Ed25519 + key-rotation; keychain behind file-fallback.
- [ ] **L1 follow-ups:** **L1-FFI** (iOS encode_pairing_code threads relay_url — fast-follow) · **L1-KS** (KeychainGroupKeyStore, GATED behind file-fallback + all-devices-upgraded = Taylor's call, live-data hazard) · **L1-OPS** (Taylor: HA relay `--admin-token` + `TESELA_RELAY_MAX_BODY=16777216` + Tailscale/WAN URL never LAN — HARD gates, silent break if unset).
- [x] **L3 spec DONE + KB1 DONE** (`phases/2026-06-14-l3-rebindable-keys-spec.md`). Registry-native rebindable keys (sparse per-command `BindingOverride`). **KB1** (data layer: store + `eventToShortcutGlyph` + effective resolvers + `checkRebind` 3-tier + `buildKeymapIndex(overrides)`; scratch outlier fixed) merged — Qwen 4.5 (head-to-head; Sonnet hit an OpenRouter-credits zero-diff). No dispatch wired (KB1 can't affect the running app).
- [x] **KB2 DONE** (Sonnet-via-Agent 5/5) — ⌘-ladder → `resolveShortcut` (call-time map read); peek/command-station runs reconciled first. **Verified byte-identical by code-equivalence** (every combo → registry cmd w/ verbatim run, none gated) + KB1 unit tests. ⚠ Live DevTools QA still owed (dev stack died session-boundary).
- [ ] **L3 KB3-4 NEXT:** KB3 (`effectiveChord` into leader-tree + effective badges in palette/colon) · KB4 (settings UI: registry-driven rebindable section). Both want live QA → restart the dev stack first.
- **⚠ DEV STACK DOWN:** vite + the web backend died across the session boundary; stale tesela-server procs linger. Restart (`pnpm --dir web dev` + `tesela-server --mosaic ~/Library/Application Support/tesela/logseq`) before browser QA.
- **Sonnet path:** OpenRouter STILL credits-blocked (pi/openrouter Sonnet + kimi = dead). **Sonnet works via the Agent tool (Claude subscription)** — that's how KB2 ran. Top up OpenRouter to restore the pi fleet's Sonnet.
- **⚠ INFRA (2026-06-14):** the **OpenRouter account is out of credits** → Sonnet-via-openrouter AND kimi-via-openrouter both zero-diff. With gpt-5.5 rate-limited, reliable fleet paths = **qwen3.7-max (opencode-go)** + minimax (flaky). Taylor: top up OpenRouter credits to restore Sonnet/kimi, else route must-land to qwen.
- **Next:** L3 KB2-4 · L1 follow-ups (FFI/KS/OPS) · L4/L5 specs · deferred CF future. Fleet backlog otherwise cleared.
- **ROUTING (2026-06-13):** gpt-5.5 OUT OF LIMITS → **Sonnet 4.6** (`pi --model openrouter/anthropic/claude-sonnet-4.6`) for must-land. **kimi-k2.7-code BACK IN ROTATION** (Taylor's call) for head-to-head comparison data — was 2/2 weak (2.0 avg) as older build; `opencode-go/kimi-k2.7-code` (auth ok). minimax flaky under load.
- **Evergreen scorecard PUBLISHED to harness-deck** (`~/.harness/reports/tesela/model-eval-scorecard/`, regen via `.bench/gen-scorecard-report.mjs` from model-bench.jsonl). gpt-5.5 4.85(13)·minimax 4.14(14)·sonnet 5.0(1)·kimi 2.0(2). Re-run the generator after each batch.
- [ ] **Then:** **ED1-fix** (salvage `.bench/wave3/logs/ed1.diff.patch`) · **L1** sync (HA-first — big, design = Taylor's call).
- [ ] Taylor: **H1–H4** confirms (real browser + Roshar); push when ready (18 new commits); 4 L2 keyboard Qs; PROP5 default-OFF — flip on if wanted; green-light chezmoi items.

## Scorecard tally (waves 1–5, `model-bench.jsonl` 32 rows)
- gpt-5.5 = **10/10 clean** (every item 5/5 — Rust, TS, Swift, bash; incl. the hardest UX item unaided). minimax = 8/10 quality 4.5–5 BUT **2 load-fails wave4** (prop4 errored-after-completing, prop5 zero-diff → re-dispatched to gpt-5.5). **Routing rule: minimax output is solid but it hits `2064` high-load errors under volume → gpt-5.5 for must-land/hard items, minimax for S mechanical.**

## Blockers
- None active. Roadmap "Now" STALE (Stream A/B shipped). minimax hitting 2064 high-load errors — prefer gpt-5.5 for must-land items.

## Open Questions
- L2 spec: 4 keyboard-first Qs for Taylor (editor-verb-no-block / unify slashKey+chord / typed-date-path / is-widget-a-slash-verb) — defaults baked in, non-blocking.
- CF relay deploy (needs Taylor's CF acct); desktop vs iOS sequencing.

## Notes
- Review routine proven 4×: read `_summary.txt` → per-item `diff.patch` → `git apply [--3way]` → Verify in main → commit w/ provenance → worktree remove → `model-bench.jsonl` row. Opus review IS the gate (caught ed1 runtime bug, prop2 false-flake, prop5 empty).
- Report: `phases/2026-06-13-opus-return-report.md`.
