# 2026-07-15 — Relay durability train

## Mission
- Close `tesela-gqd`, `tesela-fdd`, `tesela-zip` as one protocol/recovery change.
- Prove Rust relay + Cloudflare Worker parity, retained-op recovery, installed desktop behavior, and iOS Simulator convergence.
- Preserve the existing relay wire during migration; no push or production rollout.

## Root causes
- `gqd`: snapshot payload UPSERT is unconditional; stale `covers_seq` can replace a newer row. Chunking currently overloads `covers_seq=0` as both an inert GC watermark and each row's sequence, so a guard alone would strand non-final chunks.
- `fdd`: snapshot AEAD AAD binds only the group. Relay-supplied `stream_id` and `snapshot_seq` route plaintext but are unauthenticated.
- `zip`: terminal apply failure records the raw envelope seq and advances the normal cursor, but catch-up reads snapshots only; an oversized/missing snapshot makes the retained op unreachable.

## Plan
- [x] Protocol red tests. Add relay conformance coverage for fresh-then-stale interleaving and an inert batch carrying an explicit per-entry sequence. Verify: `cargo test -p tesela-relay --test conformance`
- [x] `gqd` green. Add explicit per-entry `snapshot_seq`; guard UPSERT by nondecreasing row seq in Rust + Worker; keep batch `covers_seq` solely for watermark/GC. Upgraded clients declare `snapshot_seq_version=1`, including empty checkpoints. Unmarked legacy requests remain accepted but GC-inert so a rejected old chunk cannot lose its healing op. Verify: `cargo test -p tesela-relay`
- [x] `fdd` red/green. Keep the legacy `OuterPayload` and group-only AEAD readable by old clients, then append an HMAC-authenticated routing record binding group + stream + writer seq. Mark v2 inside the nonce so new clients require that record and suffix stripping cannot downgrade it. Retain an explicit legacy-v1 read path and tolerate an old relay reporting batch `covers_seq` externally. Verify: `cargo test -p tesela-sync -p tesela-relay`
- [x] Preserve heal-deposit freshness. Carry each successful outbound relay seq into desktop heal-snapshot entries so an inert `covers_seq=0` request can still replace the older snapshot it supersedes. Existing FFI calls remain source-compatible; iOS inert snapshots remain seq 0 and cannot regress a newer proven row. Verify: `cargo test -p tesela-server tick_deposits_snapshots -- --nocapture` and `cargo test -p tesela-server periodic_deposit_skips_unchanged_notes_but_still_compacts -- --nocapture`
- [x] `zip` red/green. Before snapshot fallback, poll from the earliest retained seq without changing/acking the normal cursor; decode only the exact queued note+seq updates, reapply idempotently, and clear only cleanly applied targets. Preserve snapshot fallback for pending/permanent failures and bootstrap rows whose raw ops are already gone. Verify: `cargo test -p tesela-server retained_op -- --nocapture` and `cargo test -p tesela-server tick_holds_cursor_at_failed_apply_then_gives_up_after_bound -- --nocapture`
- [x] Cross-relay verification. Rust + local Wrangler conformance both pass 29/29; relay/sync/server suites and Worker TypeScript pass. The required workspace formatting/lint commands were run and expose only the tracked Rust 1.96 baseline (`tesela-bz5`, `tesela-8wk`); feature crates pass scoped clippy with named baseline allowances. Verify: commands above; evidence in report.
- [x] Product QA. Canonical signed desktop install + reinstall, explicit iPhone 17 Pro Simulator build, paired two-way note edits, both relaunch paths, and repaired 2026-07-14 block shape pass. The cleanup delete exposed separate `tesela-vuw5`; no cross-note contamination was observed. Verify: captured API/UI evidence in the matching report.
- [x] Closeout after mixed-version re-audit. Refresh ADR/report/deck, re-close `tesela-gqd`, keep `tesela-fdd`, `tesela-zip`, and self-driven `tesela-bw84` closed, update handoff, and commit. `tesela-vuw5` holds the independently discovered NoteDelete gap. Verify: `git status --short`; `bd show tesela-gqd`; `bd show tesela-fdd`; `bd show tesela-zip`

## Invariants
- Older clients and relays interoperate during rollout: old relays ignore the upgraded marker; new relays accept unmarked old deposits but never let them advance GC. Older snapshot rows stay readable; new rows fail closed on stream/routing-record tampering.
- Lower sequence never replaces higher; equal sequence remains idempotently replaceable.
- Non-final chunk uploads may update snapshot rows but never move watermark or GC.
- Recovery polling never advances or acknowledges the main inbound cursor.
- A queued note is removed only after raw replay or snapshot import actually succeeds.
- No direct Markdown repair/write path in product QA; mutations flow through Loro-backed APIs/clients.

## Report
- `2026-07-15-relay-durability-train-report.md` on completion.
