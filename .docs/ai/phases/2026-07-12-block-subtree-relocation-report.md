# Block subtree relocation report

**Bead:** `tesela-b54` · **Date:** 2026-07-13 · **Verdict:** implementation and automated product verification pass; separate interactive Browser-plugin QA was unavailable because the runtime exposed no browser backend.

## Delivered

- Graphite Dailies block handles move a complete stable-BID subtree before,
  inside, or after a block in the same or another day.
- Day-header and empty/synthetic-day targets append at root depth. An absent
  ISO daily is created only by a successful atomic relocation and contains no
  placeholder blank block.
- Leader/palette move mode supports `j`/`k`, `b`/`i`/`a`, date-header append,
  and selecting-state Escape. Alt-Up/Down/Right use the same durable path.
- Invalid self, descendant, malformed, external, client-minted, or ambiguous
  targets fail closed without a move request.
- Successful moves preserve bids, descendant order/depth, typed properties,
  focus, Markdown materialization, Loro projection, and reload behavior.
- Retry-safe post-intent failures retain the exact request and move id; affected
  editors stay inert until `r`/`Enter` retries. Escape cannot cancel a submitted
  durable command.
- Save-admission leases remain active from the first pending edit through every
  in-flight predecessor, queued successor, teardown, or whole-note fallback.
  Failed queues remain admitted so relocation fails closed instead of outrunning
  an uncertain save.

## Durability and ordering

- `SyncEngine::relocate_subtree` owns validation, deterministic per-note lock
  ordering, crash intent/receipt persistence, destination-before-source
  durability, checked snapshots/materialization, index repair, idempotent
  replay, and recovery.
- The HTTP route validates stable locators and ISO-daily creation rules, then
  runs the normal post-write projection/event tail for changed notes.
- Before POST, the browser freezes property/block writers, settles mounted
  writers, drains every active per-note save lease, and authoritatively probes a
  locally synthetic destination after that drain. Existing affected docs then
  flush their Loro deltas through a connection-local server barrier; only an
  exact server `404` may omit a still-absent synthetic destination. A late HTTP
  write reservation captures direct writers admitted during preflight.
- The browser never authors a client-side copy/delete or optimistically removes
  the subtree.
- Focus restoration is lease-owned from before transport. Only trusted user
  input can revoke it; delayed synthetic Journal input cannot steal focus from
  the moved destination editor.

## Automated evidence

| Gate | Result |
|---|---|
| `cargo build --workspace` | pass; seven existing Loro deprecation warnings |
| `cargo test --workspace` | pass |
| `cargo test -p tesela-sync engine::loro_engine::tests::relocation` | pass; 61/61 ownership, placement, recovery, replay, duplicate-BID, crash, and convergence cases |
| `cargo test -p tesela-server --test block_subtree_move` | pass; route, absent daily, errors, replay, and post-write behavior |
| `pnpm --dir web check` | pass; 0 errors, 48 existing warnings |
| `pnpm --dir web test:unit` | pass; 970/970 |
| `pnpm --dir web test:e2e` | pass; 12/12 in 47.7s, including trusted native cross-day before/inside/after gestures, keyboard/day append, retry freeze, immediate-edit races, reload, collaboration, synthetic-day probing, and focus |
| E2E teardown audit | pass; no retained server/Vite process group or temporary mosaic after success, failure, or direct SIGINT |
| adversarial review | final independent re-audit found no release blocker in the probe-based ordering |

The E2E drag helper begins with a trusted pointer threshold move before
scrolling the distant target. Playwright's default long-span `Locator.dragTo`
scrolls the target before Chromium emits `dragstart`, so it recorded zero drag
events and was not valid evidence for this interaction.

## Baseline-only quality-gate drift

- `cargo fmt --all -- --check` still exits 1. Isolated archive comparison with
  rustfmt 1.9.0 found 173 formatter hunks across 42 files at base `d12e6d6b`
  and 168 at the feature head. Branch-touched Rust files now pass targeted
  `rustfmt --check`; the remaining workspace complaints are baseline-only.
  Existing normalization bead: `tesela-bz5`.
- `cargo clippy --workspace -- -D warnings` exits 101 on three warnings in
  untouched baseline files: `type_complexity` at
  `tesela-core/src/db/sqlite.rs:110` and `:809`, plus `unnecessary_sort_by` at
  `tesela-core/src/nlp_lift.rs:698`. Follow-up: `tesela-8wk`.

## Residual verification and follow-ups

- The held live QA server started and cleaned up correctly, but the Browser
  plugin returned an empty backend list. No separate interactive click-through
  is claimed; the Chromium-rendered Playwright suite is the product evidence.
- `tesela-crh`: serialize or isolate concurrent web E2E runs. Until then, run
  web check/unit/E2E commands serially because worktrees share generated state.
- `tesela-psl`: reservations are tab-local; a different browser tab can still
  admit a writer after this tab's preflight. Duplicate-pane
  teardown/successor handoff is covered by source-order and unit invariants,
  but not its own behavioral E2E.
- Concurrent same-root moves to different destination notes remain explicitly
  out of v1 scope per the approved spec.
