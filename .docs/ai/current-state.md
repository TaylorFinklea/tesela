# Current State

> **Backlog/ready-queue → beads (`bd ready`) as of 2026-06-30 (pilot).** New actionable work is filed in beads, not roadmap Now or this file. `.beads/` is git-excluded (stealth, local-only); decisions/phases/loop-state stay prose. See AGENTS.md → "Task tracking — beads pilot".

## Loop state — fresh context: start here, then `bd ready`
- **Backlog/what-next is in beads.** `bd ready` (priority queue) → `bd show <id>` → `bd update <id> --claim` → do it, run the repo Verify (build/test) → `bd close <id> --reason "…"`. `bd create` for work discovered mid-task. **I own queue accuracy — close completed items myself; don't ask Taylor to.**
- **Branch `main` — LARGE unpushed stack** (the whole sync/presence/iOS arc below, `cf212bee` → build-66 bump `7c89f944`). **Remind Taylor to push; I never push.** `.docs/ai/review/` + `.beads/` untracked; `AuthKey_*.p8` gitignored — NEVER commit.
- **Latest iOS = TestFlight build 66.** Desktop installed + serving (random loopback port — see landmines). Taylor is sole tester; cut iOS builds freely.

## DONE — north-star arc: multi-device sync + presence + iOS NLP/capture (2026-06-29/30, builds 56→66)
Verified LIVE by Taylor across iPhone/iPad/desktop ("Holy crap it works"). Full detail in git + closed bd items; summary:
- **Sync convergence (the big one).** Garble (concatenated block text under concurrent edits) = a device authoring on a fresh DISJOINT Loro lineage. Fixed by **bootstrap-before-author**: import the relay's authoritative note as a shared base before the first `record_local`, wired into ALL server authoring entries incl. create paths (`285bc557`, `74fb4689`). Separate **deposit-strand** bug (broadcast cursor ahead of current `oplog_vv` → empty delta → no `PUT /ops`, phone edits stranded) fixed with a snapshot fallback, self-healing (`10aafd6c`, build 57). Also: past-day heal `cf212bee`; bootstrap-when-behind + relay `X-Tesela-Compaction-Seq` `131a1039`; convergent idempotent `write_block_text` `8171b0b8`.
- **Presence.** Device-labeled remote cursors; iOS block-level chips + peer-color block tint + un-clipped name flags; per-day cursors in the multi-day journal (`e5b5b90c`, `68c8fc18`, `3293861b`, `61ff7d13`). CF Worker presence relay deployed by Taylor. Compact capture sheet (a keyboard-avoidance over-correction was reverted, `319e14bb`).
- **iOS NLP / slash / capture parity with web.** Slash `/p1`, inline NLP (auto-lift on blur), and live token coloring now on EVERY surface — today, pages, past-day, and Capture (which gained a type/tag picker + add-time NLP). Capture & blocks share ONE resolver `PropertyRegistry.effectiveLiftRegistry` (builtins fallback ⇒ NLP works pre-sync; live wins when liftable) so they can't drift (`2ecb51e7`, `ec3d39aa`, `63255ad4`, `0c2bc21d`, `7c89f944`). Bare TRAILING dates lift the deadline (no "due" needed; mid-prose still requires intent word).
- **Hardening audit** (find→verify, 14 confirmed): fixes shipped; deferred items filed in bd (qql/fkg/9je/9t0/ug7 etc.).

## Landmines / how-to (don't re-learn)
- **Desktop redeploy when WEB (`web/src`) changed:** `cargo tauri build` does NOT rebuild the web (no `beforeBuildCommand`; `frontendDist=../web/build`). Use `scripts/build-desktop.sh` (npm build → cargo tauri build → install-desktop.sh) OR `cd web && npm run build` first — else a STALE web bundle ships (bit hard 2026-06-30). Rust-only change → plain `cargo tauri build` + `bash scripts/install-desktop.sh` is fine. (Memory: project_desktop_web_build_gotcha.)
- **Desktop server binds a RANDOM 127.0.0.1 port** (`TESELA_SERVER_BIND=127.0.0.1:0`), NOT 7474. install-desktop.sh probes the real port via lsof. Check live: `lsof -nP -p $(pgrep -f Tesela.app/Contents/MacOS/tesela-desktop|head -1)|grep LISTEN`.
- **iOS:** `scripts/ios-testflight.sh` rebuilds the Rust FFI for iOS + xcodegen + archive + auto-bumps `CFBundleVersion` in project.yml + uploads. SourceKit shows phantom cross-file / `No such module UIKit|XCTest` / `Cannot find type` errors — **trust `xcodebuild`, not the editor** (memory: project_ios_sourcekit_false_positives).
- **Dead branch `wip/concurrent-convergence-shared-base`** = the abandoned deterministic-seed approach (bootstrap-before-author / "Option A" won). Deletable.

## Shipped specs (historical — work done)
- `phases/2026-06-29-concurrent-convergence-spec.md` (convergence — shipped as Option A bootstrap-before-author).
- `phases/2026-06-29-ios-surface-parity-spec.md` (iOS slash/NLP/presence on pages+past-day — shipped).
- `phases/2026-06-27-relay-presence-spec.md` + `2026-06-27-multidevice-presence-spec.md` (presence — shipped).
