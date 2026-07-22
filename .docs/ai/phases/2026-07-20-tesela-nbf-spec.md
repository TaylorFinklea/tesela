# tesela-nbf — Immutable Node / Relation Properties

Status: **IMPLEMENTED — verification complete except repository-wide pre-existing Clippy/TUI and parallel server shutdown gates; Bead remains open.**

Frozen 2026-07-20 after Taylor approval and mandatory Opus 4.8 architecture approval. Recovery proceeded with one writer. The original partial diff remained recovery material until independently audited and tested.

## Recovery boundaries

- One active writer in `/Users/tfinklea/git/tesela`; reviewers are read-only.
- Preserve without staging or committing: `crates/tesela-server/src/routes/commands.rs`, `crates/tesela-server/src/routes/views.rs`, `.agents/`, `.codex/`, `.harness/`, `.pi-subagents/`, and `dist/`.
- Never use `git stash`, `git reset`, `git checkout`, or `git clean`. Do not inspect `target/` or `dist/`.
- Inspect, retain only what satisfies this contract, and correct or remove recovery material that does not. Do not infer acceptance from the partial diff or agent output.
- Each missing behavior or defect gets a discriminating RED test before its production change where feasible.

## Immutable PageId authority

- `PageId` is a canonical UUID persisted in each note’s Loro root and mirrored in reserved frontmatter key `tesela_page_id`.
- On creation, import, restore, and old create-copy flows: read root and frontmatter first. If neither carries an ID, derive UUIDv5 under a fixed Tesela namespace from the note’s **current legacy Loro document-address bytes**, then persist root and frontmatter immediately.
- Once persisted, PageId is never recomputed from a slug or document address. Existing slug `NoteId`, filenames, routes, relay stream IDs, cursors, snapshots, and `NoteStore` contracts remain unchanged.
- Root/frontmatter/directory disagreement, malformed IDs, duplicate live PageId bindings, and incompatible forwarding are explicit repair/conflict states. Resolution fails closed; no cache, title, slug, or alias heuristic may silently rebind a relation.

## Synced page directory

- A reserved page-directory Loro document is synced, persisted, snapshotted, restored, and relayed like an ordinary document while never becoming a note.
- Directory records use flat scalar keys/records. Do not create fresh nested containers under deterministic PageId keys.
- Before normal record writes, establish every immutable `(PageId, legacy-document-address)` binding with a byte-identical update authored by a deterministic reserved peer. Distinct bindings must have distinct seed identities; repeated seed import is idempotent.
- Concurrent first creation of the same PageId binding must converge to identical operation IDs and field-wise merge mutable slug/title/deleted/forwarding data. Concurrent bindings for distinct pages must not collide.
- Directory values are repairable resolution state only. They must never overwrite note-root content from a stale cache. Old aliases and deleted/forwarding targets remain as provenance/tombstones.

## Special-document safety

Before the directory is added, replace every hard-coded Views-only test with extensible `is_special_doc` / `SPECIAL_DOC_IDS`, including:

1. materialization and render walks;
2. derived index rebuild and note count;
3. note-shaped operation apply;
4. twin scan/heal;
5. relocation, including every inline `VIEWS_DOC_ID` comparison; and
6. all remaining Views-only comparisons in the Loro engine.

Add direct tests proving the directory is excluded from each category. Backup authority tests must explicitly assert the directory snapshot filename, not merely recursive `.tesela/loro/` inclusion.

## Rename and identity continuity

- Existing rename remains in scope: it is create-copy/delete, not an in-place slug update.
- Before tombstoning old state, write the new document with the original PageId and atomically publish forwarding/alias resolution from old document/slug to new document/slug.
- Relations remain attached to PageId through the overlap and resolve to the new live document after tombstone convergence.
- Incompatible concurrent renames expose a conflict/repair state and fail closed. Do not migrate relay stream IDs or public slug URLs.
- Add a regression proving a Node relation survives rename and another proving incompatible concurrent targets do not silently choose a winner.

## Node storage, relation projection, and backlinks

- A Node property is one canonical target PageId in this slice. It supports block-owned and page-owned property containers through existing typed CRDT property operations; do not alter property-container topology.
- Picker writes store canonical PageId only. Existing non-UUID Node strings remain visible and unresolved until an explicit picker selection; never silently coerce or rebind them.
- Materialize a separate rebuildable `relation_edges` projection with source PageId, optional source block ID, property key, and target PageId. It is independent from existing wiki-link `links`; do not change or overload wiki-link storage.
- Backlink UIs merge relation and wiki-link references additively. Relation entries identify source page, source block when present, and property key.
- Deleted, unresolved, and conflict targets remain visible, explicitly labeled, and non-clickable.

## JQL semantics

- Tokenize balanced `[[My Project]]` as one WikiLink RHS token, preserving its wrapping through parsing. A bare `[[...]]` in predicate position emits a diagnostic and never becomes empty match-all.
- Only a Node-typed predicate resolves wrapped RHS. Existing `has-link` syntax and precedence remain unchanged.
- Add `QueryContext` / resolver APIs additively. Existing boolean matcher APIs remain compatibility delegates.
- Node `=`, `!=`, `IN`, and `NOT IN` resolve `[[title or slug]]`, bare or quoted legacy saved-view title/slug values, aliases, and raw PageId. Exact unique slug wins before exact unique title/alias.
- Ambiguous, deleted, malformed, or unresolved RHS fails closed for every operator and emits diagnostics.
- For a resolved target, retain existing missing-property behavior: absent property matches `!=` and `NOT IN`; authors use `has project` to exclude absence.
- Existing saved views keep working through RHS resolution; canonical newly authored Node predicates emit `[[...]]`.

## Shared cross-engine fixture

- Relation context lives at **case level**, never inside strict `block`.
- Update Rust, web, and iOS consumers together. Every Node case asserts that context exists and was decoded; ignored, misspelled, or omitted context fails loudly.
- The fixture stays block-kind-only. Page-owned Node property behavior has dedicated Rust, web, and iOS tests.
- Cover multi-word title, slug, alias, ambiguity, missing/deleted target, raw PageId, `=`, `!=`, `IN`, `NOT IN`, missing property, bare-wikilink non-match-all, and `has-link` regression.

## Web and iOS product contract

- Offer `Node` in property configuration.
- Reuse the existing page-search ranking/vocabulary for Node selection; do not create a second candidate corpus.
- Each client has a native searchable picker with Save, Cancel/Escape, and no write on cancellation.
- Node chips show a resolved current title and navigate PageId → current slug only when resolution is unique and live.
- Both clients expose explicit unresolved/deleted/conflict states and additive relation backlinks.
- Verify relaunch and sync persistence through applicable HTTP, relay, and mock/on-device paths.

## Execution phases

0. **Architecture gate:** formal freeze, Taylor approval, and fresh Opus 4.8 architecture APPROVE.
1. **Identity recovery:** audit retained diff; finish persisted PageId, directory, special-doc exclusions, and backup/restore proof.
2. **Rename recovery:** finish forwarding, aliases, tombstones, conflict behavior, and relation-survival proof.
3. **Relations recovery:** finish typed Node storage, `relation_edges`, backlinks, JQL, and shared fixture.
4. **Web recovery:** finish reusable-search picker, render/navigation states, page-owned editing, backlinks, and tests.
5. **iOS/FFI recovery:** finish real FFI-backed search/resolution, picker/render/backlink states, page-owned behavior, tests, and simulator verification.
6. **Completion:** independent diff audit, adversarial review, product QA, handoff/ADR/report, Bead closure, and scoped commits.

## Required verification

Run focused gates before serial full gates:

```text
cargo check -p tesela-sync
cargo test -p tesela-sync page_directory
cargo test -p tesela-sync rename
cargo test -p tesela-backup --test authority_capture
cargo test -p tesela-server --test restore_drill
cargo test -p tesela-core
cargo test -p tesela-sync
cargo test -p tesela-sync-ffi
cargo test -p tesela-server
bash scripts/check-ffi-drift.sh
pnpm --dir web test:unit
pnpm --dir web check
focused iOS XCTest and simulator build
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --workspace
```

Review and product claims require independently observed command/browser/simulator output, never subagent reports alone.

## Completion record

- Update this spec and write the matching completion report.
- Record the durable PageId, directory, and rename decisions in `.docs/ai/decisions.md`.
- Update `.docs/ai/current-state.md` and roadmap; close `tesela-nbf` only after the gates pass.
- Commit only explicit feature paths; never push.
- Deliver a manual QA checklist covering web/iOS selection, Save, Cancel/Escape, rename survival, unresolved/deleted/conflict states, relation backlinks, JQL, sync/relaunch persistence, and adjacent wiki-link/type-registry regressions.
