# Relay durability train report

**Beads:** `tesela-gqd` · `tesela-fdd` · `tesela-zip` · `tesela-vuw5` · **Date:** 2026-07-15 · **Verdict:** implementation, shared relay conformance, installed desktop QA, native iOS Simulator QA, and cross-device NoteDelete durability pass.

## Delivered

### Stale snapshot rejection (`tesela-gqd`)

- Snapshot requests carry an optional per-entry `snapshot_seq`; legacy requests
  inherit the batch `covers_seq`.
- Upgraded requests declare `snapshot_seq_version=1`, even when the snapshot
  list is empty. Unmarked legacy requests stay wire-compatible but cannot move
  the watermark or GC; this closes the mixed-fleet hole where a rejected old
  non-final chunk could otherwise be followed by a compacting empty checkpoint.
- Rust SQLite and Cloudflare Durable Object UPSERTs replace a row only when the
  incoming sequence is equal or newer.
- Batch `covers_seq` remains the independent group watermark and GC boundary.
  Non-final chunks therefore upload with `covers_seq=0` while their entries
  retain the sequence they actually cover.
- Desktop broadcast-heal deposits retain the successful outbound relay
  sequence per note and group snapshots by that sequence. Their requests stay
  compaction-inert without becoming stale-at-zero.

### Authenticated snapshot routing (`tesela-fdd`)

- New snapshots preserve the legacy postcard `OuterPayload` and group-only
  AEAD at byte zero for old-client readability.
- A nonce marker makes a trailing routing record mandatory to new clients. Its
  HMAC binds the group, length-prefixed stream id, writer sequence, nonce, and
  ciphertext.
- Stream swaps, sequence-record tampering, malformed suffixes, and suffix
  stripping fail closed. Unmarked old rows retain an explicit legacy read path.
- A new client tolerates an old relay reporting its batch watermark externally;
  the authenticated writer sequence remains inside the verified record.

### Retained-op recovery (`tesela-zip`)

- Catch-up first polls the exact raw op range protected by
  `catchup_since_seq`, without changing or acknowledging the main cursor.
- Only updates whose note id and relay sequence exactly match the queue are
  re-applied and fanned out. A note leaves the queue only after a clean apply.
- Snapshot import remains the fallback for causal gaps, permanent failures,
  compacted bootstrap rows, and any target the raw replay cannot heal.

## Red-green proof

| Regression | Before | After |
|---|---|---|
| Interleaved snapshot deposit | late seq 5 replaced the retained seq 12 payload | both relays retain seq 12 and the newest payload |
| Legacy chunk during rollout | a rejected unsequenced row could be followed by a legacy checkpoint that GC'd its healing op | unmarked legacy requests are accepted but watermark/GC-inert |
| Snapshot routing | a group-valid ciphertext could be presented under another stream | stream swap, route-seq tamper, and suffix stripping reject |
| Terminal apply failure with no snapshot | note stayed absent after the retry budget although its op remained on relay | the exact retained op replays and clears the recovery queue |
| GC-inert heal snapshot | row recency collapsed to seq 0 | row carries the successful outbound relay sequence while watermark stays inert |

## Automated evidence

| Gate | Result |
|---|---|
| `cargo test -p tesela-sync -p tesela-relay` | pass; sync 299 unit tests (1 ignored) plus integrations, relay conformance 29/29 plus client/convergence/cutover/recovery/chunking suites |
| `cargo test -p tesela-server -- --test-threads=1` | pass; 108 unit tests plus all integration and doc tests |
| Shared conformance against local Wrangler Worker | pass; 29/29, including stale-row, interleaved inert-chunk, and legacy-GC-inert migration coverage |
| Worker TypeScript `tsc --noEmit` | pass |
| `pnpm --dir web check` | pass; 0 errors, 48 pre-existing warnings |
| `pnpm --dir web test:unit` | pass; 978/978 |
| Feature-crate clippy | pass with only named allowances for tracked baseline warnings |

The broad parallel server run observed one connection-reset flake in an
unchanged spawned-process test. Its focused rerun passed, and the full server
suite passed serially. No assertion was weakened.

## Installed product QA

| Surface | Evidence |
|---|---|
| Desktop build/install | canonical `scripts/build-desktop.sh` release build installed `/Applications/Tesela.app`; canonical reinstall/relaunch also passed |
| Desktop integrity | Apple Development signature passed strict deep `codesign` verification; embedded health and relay status were healthy with no relay error |
| iOS build | Debug app built and launched on explicit iPhone 17 Pro, iOS 26.5, simulator `FDDFB511-272B-40DD-8927-5E71311E96BA` |
| Existing mosaic | recovery flow paired the simulator; existing dailies loaded from the same sync group |
| `tesela-bw84` gate | 2026-07-14 displayed the repaired task metadata and exactly one restored child on iOS; desktop API proved the same shape; both relaunches retained it |
| Desktop to iOS | desktop created a disposable note; native iOS Search found it and displayed the seed block |
| iOS to desktop | iOS added a block; the installed desktop API received it |
| Desktop return trip | desktop added a second block; iOS foreground polling rendered it |
| Persistence | iOS stop/launch and desktop canonical reinstall/relaunch retained all three blocks |
| Isolation | no unrelated note or daily changed during the round trip |

The desktop WKWebView was not attachable through the available local GUI
automation backend, so desktop proof used the installed bundle's live API,
health endpoint, signature, and process lifecycle. The iOS half used the native
Simulator UI for Search, block authoring, repaired-note inspection, and relaunch.
Simulator automation sometimes backgrounded the app; unified logs showed normal
`appDidSuspend` events and no crash report, fatal error, or abort.

## Resolved follow-up (`tesela-vuw5`)

The original delete removed the note from both the resident-doc map and slug
index before `produce_relay_updates` enumerated candidates, so the authoritative
peer had nothing left to export. Note deletion now commits a sticky
`root.deleted=true` value inside the per-note Loro document and retains its
snapshot. Rendering, indexing, note counts, and Markdown materialization treat
that tombstone as absent; the relay still has a durable delta to deliver.

The regression `relayed_note_delete_remains_absent_after_both_peers_restart`
proved RED at zero produced updates, then GREEN across two authoritative peers
and both engine reopenings. Installed-product QA used the final desktop bundle
and rebuilt iOS Simulator app: a fresh desktop note materialized on iOS;
desktop DELETE returned 204; relay `last_put_at` advanced from `1784169961` to
`1784170816` with `last_error: null`; iOS removed the file; desktop returned
404 after canonical reinstall/relaunch; and iOS retained absence after
terminate/relaunch. The stale simulator-only note from the original failure was
then recovered and deleted through the repaired path, leaving no QA note behind.

That recovery POST returned 500 after successfully materializing its remote
snapshot, exposing a separate non-atomic create/bootstrap path. It is filed as
`tesela-rp65`; the subsequent NoteDelete completed and relayed normally.

## Baseline-only quality gates

- `cargo fmt --all -- --check` still reports the tracked workspace formatter
  normalization backlog (`tesela-bz5`). The change did not normalize unrelated
  files.
- Strict repository clippy remains blocked by Rust 1.96 baseline findings in
  unchanged code (`type_complexity`, `unnecessary_sort_by`, plus documented
  doc/deprecation lints). `tesela-8wk` owns normalization; the three feature
  crates pass scoped clippy with those named baseline allowances.
