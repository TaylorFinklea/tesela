# 2026-06-13 — Opus return: recon + smoke-test report

Opus back as orchestrator. Goal: bearing on state, issues + corrective actions, smoke web + iOS,
then product session → tiered backlog → fleet dispatch. Model-eval evidence is a first-class goal
([[project_model_eval_primary_goal]]); ledger = `.docs/ai/model-scorecard.md`.

## Build health (verified this session)
- **Web** ✅ svelte-check 0 errors (43 pre-existing warns), 315/315 unit, build clean.
- **iOS** ✅ `xcodebuild` BUILD SUCCEEDED (iPhone 16 sim, Debug); project `app/Tesela-iOS`, scheme `Tesela`.
- **Rust** ⚠️→ mostly green. `cargo build`/`clippy`/`fmt` clean. **Flaky integration tests** in `tesela-server`: pass in isolation, fail intermittently under parallel `cargo test --workspace`. Two modes fixed in `08f8448` (validation-window race + 60s bind timeout); residual **port-collision** race (parallel `#[test]` race the `pick_free_port` drop→rebind window) → backlog item below. 3× workspace runs after the fix: 2/3 green (1 fast port-collision fail in `views_registry_routes`).

## Web/desktop smoke (Chrome DevTools vs a COPY of the real 522-note mosaic, relay OFF on :7474)
- ✅ Loads, renders full journal timeline, **0 console errors/warns**.
- ✅ **⌘K palette**: registry-driven (~44 cmds), fuzzy filter, shortcuts shown, Esc closes + restores focus. (B1–B3 spine working.)
- ✅ **Leader (Space)**: which-key tree, submenu drill (`g`→go-to a/A/c/d/D/g/h/i/I/t/y), registry chord metadata, Esc back.
- ✅ **Editor round-trip**: `i`→INSERT→type→Esc→autosave→**persisted to Loro** (verified via `GET /notes/2026-06-13`). Bid marker hidden in editor but present in storage → Codex `30520ad` fix working.
- ❌ **Colon (`:`) ex-mode does NOT trigger in the Journal view** (leader works from same context; colon doesn't). One of the 4 command surfaces dead in the primary view.
- ❌ **Slash (`/`) menu does NOT trigger in journal/inline editors** (start-of-block or mid-line). Slash is still ad-hoc in `BlockEditor`, not wired into JournalView, not unified into the registry.
- ⚠️ Markdown render: `#`/`##` headings, `|…|` tables render as raw text inside bullets (known: render deferred).
- ⚠️ Rail TASKS widget shows literal "Placeholder task (views phase)".
- ⚠️ Click-to-focus on journal block editors was finicky under automation (needed JS focus); likely CDP artifact since editing ultimately worked — confirm with a real click.

> **North-star implication:** the command-registry spine unified palette+leader well, but **colon + slash (2 of 4 surfaces) are broken/missing in the journal**. Finishing the registry spine (all 4 surfaces, all views) is the top keyboard-first priority.

## iOS smoke (sim shell/mock only)
- ✅ Launches; Graphite shell renders cleanly with MOCK data (`-graphite` → MockMosaicService): tag chips, task checkboxes+strikethrough, wiki-links, Daily/Agenda/Inbox/Library tabs, capture bar.
- ⏭ **Deep real-data iOS pass deferred**: needs the sim paired to a server (the known-incomplete "iOS real-data bring-up"). `idb` IS installed → can drive taps. Taylor-flagged iOS bugs (older-date daily editing dead, long-block collapse, Views populate-then-vanish) need this pass; some need the real device (Roshar) for reachability.

## Roadmap reality (docs-vs-code reconciliation — roadmap "Now" is STALE)
- **Stream A (relay hardening): FULLY SHIPPED** (A1–A14; TestFlight `202606100921`; HA 0.2.2). Roadmap shows `[ ]`.
- **Stream B (Graphite cutover): FULLY SHIPPED** (B1–B4; v4/v5 chromes deleted −4287 lines; `/g` is the only web UI).
- **Genuinely OPEN:** Milestone 3 sync spine (CF deploy [needs Taylor's CF acct], min key/pairing, cursor migration HA→CF, demote Mac-hub WS, Reminders recur-bump engine re-route, NoteDelete tombstone) — all Lead/XL (Opus). Properties P1.2–P1.9 (stalled; 3 Taylor product decisions pending on harness-deck). A10 Reminders recur-bump (partial). Desktop DMG (senior-S).
- Stale worktree `.worktrees/sync-live-debug` (0 ahead of main); live relay-connected server PID 42979 + leftover QA server PID 68365 — left untouched.

## Corrective-action backlog candidates (tiered; full items to be written in backlog phase)
- **[senior/M]** Eliminate `tesela-server` integration-test port-collision flakiness (serialize server-spawn or collision-proof `pick_free_port`). Verify: `cargo test -p tesela-server` 10× green.
- **[lead/L→ fan-out]** Finish the command-registry spine: wire **colon** + **slash** as registry dispatchers across all views (esp. Journal). North-star priority. Opus specs; fleet implements sub-slices.
- **[junior/S]** A4 follow-up: `backup_scheduler.rs` use `DEFAULT_DAILY/WEEKLY/MONTHLY` + fix module doc.
- **[junior/S]** Delete dead `web/src/lib/command-context.svelte.ts`; B2/B3 kimi cleanups (BUILTIN_SLASH_CHORDS index; `buildChordTree`→`available()`; ColonCommandLine `.available(ctx)` + Esc focus restore).
- **[junior/S]** One-shot `cargo fmt --all` cleanup commit (recurring drift).
- **[senior/S]** Desktop DMG bundling fix.
- **[lead]** iOS real-data smoke pass (pair sim→server; verify older-date editing, block collapse, Views persistence) — some needs Roshar.

## Orchestration tooling (confirmed)
- **Parallel fan-out:** `PROMPT=$(cat .docs/ai/loop-prompt.md); pi --model <m> --approve --no-session -p "$PROMPT" > /tmp/fanout-<m>.log 2>&1 &` for `openai-codex/gpt-5.5`, `opencode-go/minimax-m3`, `opencode-go/kimi-k2.7-code`.
- **Serial phase-loop (Verify gate enforced):** `RALPH_PI_MODEL=<m> ralph -t pi -n N` (ralph supports claude|codex|pi only; default model `opencode-go/minimax-m3`).
