# Block-level sync (planning doc)

## Why this exists

Phase 1.5 sync is "NoteUpsert ships the whole markdown blob, last writer
on the whole note wins by HLC." That's enough for single-device-at-a-time
use but loses data the moment two devices touch the same note in the
same sync interval. The loser's edits go into the oplog and stay there,
but they never reach disk on either device.

The fix is block-level sync: the unit on the wire is a block, not a
file. Concurrent edits to different blocks within the same note both
survive. Concurrent edits to the same block still resolve by HLC LWW,
but that's a much smaller surface than "same file."

This doc is the planning artifact for that work. It is not an
implementation order to start executing right now. It exists so we can
decide what to take next, and so when we do start, the rough shape is
already pinned.

## What "block" means here

In Tesela's outliner, a block is one bullet line: `- some text`. Blocks
nest by indent. The note's content is a tree of blocks plus a
frontmatter section above them.

A block has:

- A persistent UUID, stable across edits, devices, and re-parses.
- A parent block (or None if it is a top-level child of the note).
- An order position among its siblings.
- An indent level (derivable from depth in the tree).
- Text content (markdown text, no leading bullet or indent).

The frontmatter is treated as one block-shaped record per note for the
purposes of sync, even though the on-disk form is a YAML block. Trying
to break the frontmatter into per-field ops is out of scope for this
plan.

## The five decisions

### 1. Where do block UUIDs live on disk?

Three candidates:

- **Inline HTML comments per block.** `- some text <!-- bid:01h7... -->`.
  Survives copy-paste between notes (with caveats), human-readable,
  versions cleanly. Cost: ugly in the raw file; copy-paste between
  devices that don't strip the comments creates duplicates.
- **Side table only.** `block_index(note_id, line_range, block_id)` in
  the mosaic SQLite. Clean file, but the table has to be rebuilt on any
  out-of-band edit and we lose the ability for a peer that only got
  the .md to know about block identity.
- **Both.** Write the comment as the source of truth on disk; cache the
  mapping in the side table for fast lookup.

**Recommendation: Both, with the comment as the authority.** The cache
makes apply fast (no parse every time). The comment makes the system
robust if the cache is rebuilt from .md (someone restored from backup,
or git-cloned a mosaic).

### 2. How do producers emit Block ops?

Current write path in `crates/tesela-server/src/routes/notes.rs`:
PUT /notes/{id} writes the file, updates the index, then calls
`record_sync_upsert` to push a `NoteUpsert` op.

Block-level options:

- **A. Diff-against-prior.** Parse the prior file, parse the new file,
  diff block-by-block (by block_id), emit `BlockUpsert` / `BlockMove` /
  `BlockDelete` for what changed.
- **B. Per-mutation ops at the API layer.** Add explicit "edit one block"
  / "insert block" / "delete block" endpoints; clients call those and
  each emits one Block op. Whole-file PUT stays as a fallback that does
  the diff approach for backward compat.
- **C. Fully via the Mutation funnel.** All writes go through a unified
  `Mutation::apply` that produces both file change and ops atomically.
  Big rewrite.

**Recommendation: A first, B for primary edit paths once the web client
catches up, C deferred indefinitely.** Diff-against-prior gives us
correctness right now without rewriting the API. As the web client gets
finer-grained APIs (Cmd+Enter splits, indent / outdent leader chords),
those paths can swap to (B) for cleaner ops on the wire. The full
funnel (C) is months of work and the visible win over (A+B) is small.

### 3. How does materialize handle Block ops?

The current `SqliteEngine::materialize` writes whole files for
NoteUpsert and unlinks them for NoteDelete. For Block ops it would
need to:

1. Locate the note's file by note_id (need a note_id to slug lookup,
   probably from oplog NoteUpsert history).
2. Parse the file into a block tree.
3. Locate the target block by block_id (HTML comment in the line).
4. Apply the op (replace text / move position / remove).
5. Re-serialize the tree to markdown and write the whole file back.

This is a "parse, modify, write" cycle per op. Multiple ops in the
same envelope can be batched: parse once, apply all touching the same
note, write once.

**Open question: should materialize own the parser, or should
tesela-core expose a parse / serialize API and sync call into it?**
Answer is almost certainly the latter (sync should not duplicate the
parser), but it means tesela-sync gains a dep on tesela-core's block
parser. That's already partially the case for the schema migration,
so the boundary is workable.

### 4. Backward compat with NoteUpsert ops in the wild

Phase 1.5 already shipped NoteUpsert with a full markdown blob, and a
smoke-tested deployment exists. Block ops can't be a clean break.
Options:

- **Coexist forever.** Both NoteUpsert and Block ops are first-class.
  Receivers apply either. Producers emit Block ops, but NoteUpsert
  is still valid (e.g. for an initial bulk import).
- **NoteUpsert downgrades to "decompose on receive."** Producer never
  emits NoteUpsert after the upgrade; receivers that see NoteUpsert
  parse it into Block ops at apply time and store those.
- **Versioned upgrade.** Bump schema_version, NoteUpsert becomes
  obsolete in v2, translator chain handles old ops.

**Recommendation: Coexist forever for NoteUpsert as "full sync of a
note's blob," with block ops for incremental edits.** Cleanest mental
model: NoteUpsert = "here is the canonical state of this note's text,
discard everything else and use this," Block ops = "delta from a
previously-known state." Initial sync from a brand-new peer ships
NoteUpsert per note; ongoing edits ship Block ops.

### 5. Conflict semantics

Per-block HLC LWW is the default. Same as the current per-note rule,
just at a smaller grain. Concurrent edits to different blocks both
survive. Concurrent edits to the same block still has a loser.

Surfacing the loser is a Phase 3 problem: the loser's BlockUpsert is
in the oplog forever, so a future "conflict drawer" UI can show it as
"there was an alternate version of this block from device X at
timestamp Y." Not in scope for the first cut.

True CRDT merge (Automerge per-block) is the next step beyond that.
Out of scope for this phase. The Automerge dep is large (~300KB
compiled, RGA backbone) and not worth taking on until block-level LWW
proves insufficient in practice.

## Order of operations when we execute

1. Parser API in tesela-core: `parse_note(content: &str) -> NoteTree`
   where `NoteTree` is a tree of `Block { id, text, children }`.
   Generate UUIDs for blocks that don't yet have an HTML-comment id.
2. Serializer: `NoteTree -> String` round-trips. Tests: parse-then-
   serialize is identity for files that already have block ids.
3. Migration pass at indexer startup: open every .md, parse, write
   back with stamped block ids. Idempotent on second run.
4. Diff: `diff(old: &NoteTree, new: &NoteTree) -> Vec<Op>` producing
   BlockUpsert / BlockMove / BlockDelete.
5. record_sync_upsert in tesela-server wraps the diff: read old file
   from disk before write, parse both, emit Block ops. NoteUpsert
   is still emitted for new files (no prior version) and for the
   first sync to a peer that has never seen this note.
6. Materialize in tesela-sync: parse the target file, apply Block op,
   write back. Locate the note via a note_id-to-slug lookup that
   walks the oplog for the most recent NoteUpsert (or BlockUpsert
   carries note_id, which is good enough).
7. Convergence tests: extend `tests/convergence.rs` with cases for
   "concurrent edits to different blocks both survive," "delete in one
   place while edit in another," and "ordering of moves and inserts."

## Scope check

| Item | In | Out |
|------|------|------|
| Block-level oplog ops emitted from server writes | yes | |
| Block ids embedded in markdown | yes | |
| Parser / serializer round-trip | yes | |
| Convergence tests for block-level cases | yes | |
| Conflict UI showing alternate-version blocks | | yes (deferred) |
| Automerge / true CRDT | | yes (deferred) |
| Full Mutation funnel API | | yes (deferred) |
| Per-mutation API endpoints (insert / indent / delete block) | | yes (web client work after) |

## Estimated effort

- Decisions 1+2+3: small. A week of focused work for parser + diff +
  materialize, with the existing indexer parser as a starting point.
- Decision 4 (coexist with NoteUpsert): no extra effort if we do (A) +
  decompose-on-receive isn't needed.
- Convergence tests: a day on top.
- Migration pass for existing notes (stamp block ids): half a day,
  including the test that it's idempotent.

Call it a week and a half end-to-end, assuming the existing indexer
parser is reusable. If it isn't, add three to five days for a clean
block parser in tesela-core.

## What this unblocks

Once block-level sync ships:

- Two devices editing the same note in the same sync interval no
  longer loses data (unless they hit the exact same block).
- The convergence test surface gets meaningfully bigger; we'd want
  the smoke test to include concurrent-edit scenarios.
- The conflict drawer UI becomes worth building (Phase 3-ish).
- iOS / phone editing becomes much safer to enable.

## Open follow-ups not in this scope

- `--mosaic` flag exists now (Phase 1.5 follow-up shipped). Good.
- `--bind` and `--sync-interval` flags would be nice but are not load
  bearing.
- Encryption at rest, TLS pinning, mDNS discovery: all separate from
  the data model and tracked in `plan/sync-architecture.md`.
- The full Mutation funnel (decision 2 option C) is still a real piece
  of architectural work, just not a near-term gate.
