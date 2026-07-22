# tesela-nbf — Completion Report

Date: 2026-07-21
Status: implementation complete; Bead open pending repository-wide gate cleanup

## Delivered

- Immutable UUID `PageId` persisted in Loro root and reserved Markdown frontmatter.
- Synced deterministic page directory with aliases, tombstones, forwarding, conflict detection, persistence, backup/restore, and special-document exclusion.
- Restartable identity-preserving create-copy/delete rename; stale source updates remain on source lineage and semantically merge only uncontested changes into the live target.
- Typed page/block Node properties storing canonical PageId.
- Separate rebuildable `relation_edges` projection and additive relation/wiki-link backlinks.
- Relation-aware JQL context, balanced `[[...]]` token, exact-slug precedence, legacy RHS compatibility, and fail-closed diagnostics.
- Shared Rust/web/iOS conformance fixture context.
- Web searchable Node picker/chips/navigation/status/backlinks and page-directory cache invalidation.
- UniFFI directory/search/backlink APIs and iOS Node picker/chips/navigation/status/backlinks.

## Verification observed

Passing:

- `cargo check -p tesela-sync`
- `cargo test -p tesela-sync page_directory` — 19 passed
- `cargo test -p tesela-sync rename`
- `cargo test -p tesela-backup --test authority_capture` — 4 passed
- `cargo test -p tesela-server --test restore_drill` — 3 passed
- `cargo test -p tesela-core` — 468 passed, 1 ignored
- `cargo test -p tesela-sync` — 369 passed, 1 ignored; then 370 passed after the final forwarded-deletion regression
- `cargo test -p tesela-sync-ffi` — 50 passed
- `bash scripts/check-ffi-drift.sh` — bindings in sync, 5 files
- `pnpm --dir web test:unit` — 1,018 passed
- `pnpm --dir web check` — 0 errors, 45 existing warnings
- focused iOS XCTest (`PropertyEditingTests`, `QueryConformanceTests`) — TEST SUCCEEDED
- `cargo fmt --all -- --check`
- `cargo build --workspace`
- isolated server import budget regression — passed at 16.32 s
- isolated `serve_in_process` suite — 2 passed

Non-feature repository-wide blockers:

- `cargo clippy --workspace -- -D warnings` reaches existing `tesela-tui/src/app.rs` `field_reassign_with_default` and existing `tesela-server/src/sync_relay.rs` doc-continuation warnings. New sync Clippy findings were corrected.
- `cargo test -p tesela-server` passes 157 tests but the two `serve_in_process` 20-second shutdown tests time out only under full parallel load; the same suite passes in isolation (25.58 s total).
- Consequently `cargo test --workspace` was not represented as passing independently; the constituent changed-package suites and focused gates above were run.

## Review

Fresh adversarial review identified forwarding-order, deletion, conflict-import, dispatch, and cache-invalidation risks. The reviewed snapshot was stale for full-snapshot routing, persisted-baseline replay, conflict retention, and web invalidation; current code already addresses those paths. One valid gap remained: semantic replay omitted source block deletions. The merge now deletes only when the target still equals the frozen pre-rename source block, with a regression proving concurrent target edits survive.

## Residual risk

- Forwarded semantic target mutations currently surface through broad refresh rather than an effective-target-tagged binary delta; final target state converges, but an already-open editor may refresh rather than apply an exact-id splice.
- Full workspace green remains blocked by unrelated existing Clippy warnings and parallel timing sensitivity documented above. Keep `tesela-nbf` open until those required gates are clean or explicitly waived.
