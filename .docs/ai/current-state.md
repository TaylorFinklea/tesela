# Current State

## Branch
- `main` — **2 fix commits NOT yet pushed** (`e6d1d83b` sync liveness, `5c65e9d2` date chip) + a pending build-48 number + doc commit. `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit). **Remind Taylor to push.**

## Active work
- **build 46/47 device-test fix batch** (5 findings — see roadmap Now, 2026-06-24).
  - [x] **#1 iOS↔web sync drift / "data loss"** — was sync LIVENESS (backoff ballooned to ~2.3h + `.active` couldn't wake a parked loop), NOT push logic. Fixed `e6d1d83b` (cap backoff ≤60s, `wake()` both shells; `RelayBackoffTests`). decisions.md 2026-06-24.
  - [x] **#2 date chip hidden while editing** — fixed `5c65e9d2` (`BlockRow.chipVisibility`; dates ignore `isEditing`).
  - [ ] **#3 slash `/p1` deep-filter parity** — port web `flattenedSlashFilter` into iOS `SlashVerbs`.
  - [ ] **#4 inline NLP not firing** — NOT data/logic (ruled out); needs a SIM repro. ⚠ don't speculative-fix (build-47 gate). Details in roadmap Now.
  - [ ] **#5 per-type color+logo** — roadmap'd (Taylor wants later).
- **build 48 → TestFlight** cut this session (sync liveness + date chip). Awaiting Apple processing + Taylor device verify.

## Plan
- (no active phase loop — the batch above is the unit of work)

## Blockers
- None.

## Open questions / next pick
- **Taylor device-test build 48**: date chip now shows while editing? sync no longer drifts (background a while, edits converge fast)?
- **#4 NLP**: drive a sim — type `p1` in a `#Task` block — does it lift? Pins tag-presence vs detector-invocation vs build-47 gate.
- **Sync UX honesty follow-up** (roadmap): show iPhone's OWN relay URL + pending/last-push age; drop the dead `127.0.0.1` "Connected" in relay mode.
