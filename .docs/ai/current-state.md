# Current State

## Branch
- `main` — **5 fix commits + pending build-49 number/doc commit, NONE pushed**: `e6d1d83b` backoff, `5c65e9d2` date chip, `b187184b` build48+docs, `8e253e97` delete-refresh test, `594b0403` APNs scoping. `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit). **Remind Taylor to push.**

## Active work
- **build 46/47 device-test fix batch** (6 findings — roadmap Now 2026-06-24):
  - [x] **#1 sync drift / "data loss"** — sync LIVENESS (backoff ~2.3h + `.active` couldn't wake a parked loop), not push logic. Fixed `e6d1d83b` (build 48).
  - [x] **#2 date chip hidden while editing** — `5c65e9d2` (build 48), `BlockRow.chipVisibility`.
  - [x] **#6 web→iOS delete not propagating** — BACKGROUND APNs-wake gap (foreground path proven OK on 48 via sim repro). APNs registration was token-only → CF had no token post HA→CF migration. Fixed `594b0403` (build 49): relay-scoped `apnsRegistrationKey`. decisions.md 2026-06-24.
  - [ ] **#3 slash `/p1` deep-filter parity** — port web `flattenedSlashFilter` into iOS `SlashVerbs`.
  - [ ] **#4 inline NLP not firing** — NOT data/logic (ruled out); needs SIM repro (tag-presence vs detector vs build-47 gate). ⚠ don't speculative-fix.
  - [ ] **#5 per-type color+logo** — roadmap'd (later).
- [x] **#7 iOS→desktop push BROKEN → FIXED (build 51 `56d67001`).** iOS edit → ZERO relay PUT. On-device build-50 diagnostic confirmed `applied=1 sent=0`: the splice RECORDS but the outbound producer exports nothing. Root cause: `produce_relay_updates`→`export_doc_update` returned None on an un-decodable/incompatible `broadcast_cursor` and the dirty note was SILENTLY SKIPPED → stranded forever, then clobbered by the desktop's inbound. Fix: `export_doc_update` self-heals (bad cursor → full-snapshot fallback; idempotent; next PUT rewrites a fresh cursor). RED test `produce_re_emits_when_broadcast_cursor_is_undecodable`; tesela-sync 166 green. decisions.md 2026-06-25. **[ ] Taylor verify build 51:** type in a Today block → reaches desktop; "Last splice" shows sent≥1.
- **builds 48 + 49 + 50 + 51 → TestFlight** this session. Awaiting Apple processing + Taylor verify.

## Plan
- (no active phase loop)

## Blockers
- None.

## Open questions / next pick
- **Taylor verify (build 48/49):** date chip shows while editing ✓? sync converges fast on reopen (~5-10s, don't force-close)? after-sleep background sync improved (delete on web, phone asleep, reopen → current)?
- **#6 APNs caveat:** iOS throttles silent pushes — background wake is best-effort; the reliable path is foreground sync (proven ~5s on 48). If after-sleep staleness persists WITH the token registered, it's iOS throttling, not a code bug.
- **Next batch:** #3 slash deep-filter (bounded port), #4 NLP (sim repro first).
- **Sync UX honesty (roadmap):** show iPhone's OWN relay URL + pending/last-push age; drop dead `127.0.0.1` "Connected" in relay mode.
- **Repro toolkit (reusable):** seed a sim via `simctl spawn defaults write app.tesela.ios backend.mode relay` + `relay.cachedPairingCode <code from /sync/peer/pairing-code>`; drive desktop add/delete via `POST`/`DELETE /notes/{id}/blocks/{bid}`; watch CF via `npx wrangler tail` in `cloudflare-relay/`. (sim can't APNs/suspend.)
