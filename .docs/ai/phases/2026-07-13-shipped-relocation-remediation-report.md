# Shipped relocation remediation report

**Bead:** `tesela-8ig` · **Date:** 2026-07-14 · **Verdict:** Taylor verified iOS Move to on build 79. The first desktop remediation was necessary but incomplete; a WebKit-compatible drag locator fix now passes the full relocation suite and a signed worktree bundle is ready to merge/install. Physical desktop QA remains the gate.

## Root causes

- **Desktop drag, first boundary:** Tauri's native file-drop handler had to be
  disabled so WKWebView could own HTML drag/drop, but the installed build proved
  that setting alone was insufficient.
- **Desktop drag, remaining boundary:** Graphite seeded only a custom MIME type,
  immediately required WebKit to report that type back during `dragstart`, and
  canceled otherwise. A rejected start still left the in-memory move session in
  `selecting`, so every later attempt was refused. The corrected path seeds both
  the exact custom payload and a `text/plain` move-id marker, validates either
  against the active session at drop, and never starts a session when neither
  format can be written.
- **iOS `Move to...`:** Graphite and legacy context menus rendered the action
  but discarded `.moveTo`. The UniFFI surface exposed no relocation operation,
  so Swift had no durable engine path to call.
- **Physical mosaic identity:** the same hub URL and canonical path can later
  serve a replacement sync group. Path- or profile-id-only checks could not
  prove that an awaited write, WebSocket frame, background tick, or recovery
  outbox still belonged to the physical mosaic that admitted it.
- **Relocation delivery ordering:** source and destination changes could not be
  treated as delivered until one exact multi-note frame was acknowledged and
  its frame-associated version baselines were committed. Independent export,
  send, or checkpoint steps could otherwise strand half of a move.
- **Activation and engine concurrency:** profile activation, ordinary edits,
  relocation, relay ticks, APNs/background catch-up, final flush, and voice
  capture had no single process-wide admission boundary. An operation started
  on profile A could resume after an await while profile B was active.

## Delivered

### Desktop shell

- The Tauri main window disables only the native drag/drop handler with
  `.disable_drag_drop_handler()`. Tesela has no native file-drop product path to
  preserve.
- The Graphite drag path no longer treats immediate custom-MIME readback as the
  authority. It carries an opaque move-id fallback in `text/plain`, retains the
  full custom payload when available, and requires the locator to match the
  already-active internal session before submitting a drop. Ordinary external
  text/file drops cannot create that session.
- Drag-data seeding completes before the session transition. If both formats
  fail, the event is canceled while the move session remains idle, so one bad
  attempt cannot latch every later drag.
- A desktop regression test asserts the main window retains this setting.
  Complete-subtree semantics remain owned by the existing web request and Rust
  relocation engine.

### iOS product path

- UniFFI now exposes typed relocation request, placement, status, affected-note,
  and error records over `SyncEngine::relocate_subtree`. It accepts only
  canonical dashed UUIDs or exact dashless IDs, derives note identity and daily
  seeds in Rust, and returns pre-move versions for every affected note.
- Checked-in Swift and C bindings were regenerated. The drift script now
  normalizes generator-added trailing whitespace before byte comparison.
- `Move to...` is wired across Graphite and legacy block menus. The searchable
  sheet excludes the source, offers daily and page destinations, supports
  cancel, preserves one move id across exact retry, and surfaces actionable
  failures instead of silently closing or spinning.
- The service invokes the Rust engine only; Swift never composes copy/delete.
  Successful moves refresh every affected projection and wake relay delivery.
  Relay moves expose only locally resident page destinations because a missing
  arbitrary page cannot be seeded authoritatively offline.

### iOS durability and profile isolation

- Relocation owns one exclusive engine-and-transport lease. Existing engine
  work drains first, new work queues behind it, and the lease remains held
  through mutation, durable outbox staging, exact multi-note frame preparation,
  acknowledged send, and per-note baseline commit.
- The relocation outbox retains the exact request, physical group scope,
  prepared frame, and committable versions for retry/recovery. Legacy unscoped
  build-78 outbox data is rejected rather than replayed into the currently
  selected mosaic.
- Engine storage, relay cursors, and recovery state are scoped by physical group.
  Legacy unscoped engine/cursor data is quarantined instead of being adopted by
  a newly selected profile.
- `RelayTicker.shared` is the single process relay-engine owner. Foreground and
  immediate ticks, relocation, APNs/background catch-up, final background
  flush, and resume transitions share admission; timeout bookkeeping remains
  attached to the exact lease even if a non-cancellable Rust call outlives a UI
  timeout.
- Profile activation is latest-wins and fail-closed. The registry closes new
  mutation admission and invalidates the old activation before publishing a new
  active profile. Already-reserved operations drain against their original
  backend/generation; the new backend becomes editable only after refresh and
  path/group proof. Failed activation stays detached and non-writable.
- Direct optimistic mutation seams and voice capture carry admission plus
  backend-generation scope, preventing old-profile callbacks from publishing
  into the newly active mosaic.

### Server physical-identity fencing

- `/mosaics/current` returns canonical mosaic path and group id atomically, and
  mosaic switching persists the canonical target.
- iOS HTTP data-plane requests carry `X-Tesela-Expected-Group`. Every
  data-plane handler holds the current group read lease for its full execution;
  a mismatched physical group fails closed. Pair adoption holds the matching
  write lease through durable identity adoption and in-memory publication, so
  group replacement waits for admitted requests to finish.
- Successful adoption closes the current process's data plane until restart;
  only health, current-mosaic observation, and restart controls remain. A
  same-group-id/different-key adoption is rejected, so key rotation requires a
  new group ID and the HTTP group proof remains exact.
- WebSocket sessions announce and bind canonical path plus group id. Deltas and
  barriers are accepted only for the bound session identity; acknowledgements
  report the observed identity and reject path/group mismatch. Old sessions
  actively close after replacement, including while idle, and group-bound
  writes time out instead of holding adoption behind an unbounded socket send.
- Runtime scope includes both group id and group key. Existing WebSockets,
  relay polling, and presence bridges stop publishing when that exact group is
  replaced. Broadcast deltas carry their source group so old-group work cannot
  enter new-group subscribers.

## Automated evidence

| Gate | Result |
|---|---|
| `cargo build --workspace` | pass in the remediation gate run |
| Workspace Rust suites | relocation, server fencing, stale-session rejection, and recovery suites pass; the broad serial command also exposed the two known spawned-process/history flakes documented below, both green on focused rerun |
| `cargo test -p tesela-desktop` | pass; includes native drag/drop interception regression |
| `cargo test -p tesela-sync-ffi` | pass; includes complete nested relocation, replay/reopen/conflict, exact multi-note delta framing, strict ID parsing, and partial-export rejection |
| Full Tesela iOS simulator suite | pass; 573/573 in the build-79 release run |
| Focused iOS activation/admission regression set | pass; 7/7 |
| `pnpm --dir web test:unit` | pass; 976/976 |
| `pnpm --dir web check` | pass; 0 errors, 48 pre-existing warnings |
| Relocation Playwright spec | pass; 13/13, including custom-MIME rejection fallback with parent + child |
| Worktree Tauri release bundle | pass; fresh web assets embedded, Apple Development signed, strict deep verification passed |

## Pre-existing quality-gate warnings

`cargo clippy --workspace -- -D warnings` remains blocked by three known
baseline warnings in files untouched by this remediation:

- `type_complexity` at `crates/tesela-core/src/db/sqlite.rs:110`
- `type_complexity` at `crates/tesela-core/src/db/sqlite.rs:809`
- `unnecessary_sort_by` at `crates/tesela-core/src/nlp_lift.rs:698`

These are tracked separately by `tesela-8wk`; they are not relocation release
regressions.

The spawned-server relocation suite can also hit the pre-existing
`tesela-td2` SQLite history-write contention flake: one history row is omitted
while the move, response, events, and CRDT delta all succeed. It was reproduced
once in the broad gate and on a stress rerun, then passed focused reruns. The
contention-safe history allocator remains a separate P2; no assertion was
weakened for this release.

The broad workspace command also intermittently saw `shutdown_backup` exit its
spawned server non-zero; the unchanged test passed both focused reruns. This is
outside relocation delivery, did not reproduce in isolation, and is tracked as
`tesela-uy4`.

## Release artifacts

| Artifact / gate | Status |
|---|---|
| Remediation commit and merge to `main` | complete at `85d914ed` |
| Embedded-WebKit desktop hotfix | merged to `main` at `267c0312`; installer boundary fix at `9139e45d` |
| Corrected desktop production bundle | release bundle rebuilt from merged `main`; Apple Development signed with hardened runtime |
| `/Applications` installation updated | complete; installed Mach-O UUID matches the rebuilt artifact |
| Installed desktop health check | pass; one installed process and embedded `/health` returned `{"status":"ok"}` |
| Real installed-shell parent-plus-children drag between days | pending manual QA |
| iOS archive and TestFlight upload | complete; Tesela 1.1 build 79, `Upload succeeded` / `EXPORT SUCCEEDED` |
| iOS physical product test | passed; Taylor confirmed Move to works |
| Harness-deck product-test report | refreshed in place for the remaining desktop device verification |

Manual release QA now has one remaining gate: in the installed desktop app,
verify that the visible handle moves one parent with all children to another
day and that the result persists after relaunch. TestFlight build 79 already
passed the physical iOS Move-to check; the desktop-only hotfix does not require
another iOS release.
