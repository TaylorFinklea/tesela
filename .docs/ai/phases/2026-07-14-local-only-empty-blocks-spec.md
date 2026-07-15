# Local-only empty leaf blocks

Bead: `tesela-ju7` · Status: approved design, not implemented

## Incident

- Live Loro history: two replicas shared one persisted empty block ID, then independently authored different entries into that ID.
- One branch used character splices; the other later used a stale whole-block rewrite carrying legacy in-text properties.
- Loro merged them as edits to one logical block. One entry landed inside `tags:: Task`, and the separate intended row disappeared.
- Root cause: an untouched empty leaf was exposed as a reusable row on another device. The malformed tag was a downstream merge symptom.

## Product rule

Untouched empty leaf blocks are local-only until meaningful.

Meaningful means any of:

- non-empty prose
- tag, property, or task state
- at least one child

An empty structural parent remains visible and durable. An abandoned empty leaf may disappear after refresh/relaunch.

## Design

Use the shared Rust materialization boundary as the invariant:

1. Keep a newly created empty node in the creating replica's Loro document so its existing splice path has a stable target immediately.
2. Omit bare leaf nodes from rendered Markdown and derived indexing. This makes the reservation logically local-only: peer clients cannot render, focus, or reuse it.
3. Do not tombstone omitted reservations. When the creator adds meaningful state, normal materialization makes the same block ID visible.
4. Preserve an empty node when a later block is its descendant, matching the existing bare-leaf pruning semantics.
5. Apply the rule through the single full-note rendering path used by server, desktop, relay-backed iOS, and web projections; do not add divergent client heuristics.

Internal empty reservations may transit through Loro updates, but they are never user-visible on peers. This avoids a new first-keystroke creation API and its ordering races while enforcing the chosen product behavior.

## Data flow

- Device A inserts blank A → engine stores A → Device A keeps its optimistic row → materialized Markdown omits A.
- Device B receives A → its Markdown/UI omits A → Device B creates distinct blank B when the user asks for a row.
- First meaningful edits target A and B separately → both become materialized → normal CRDT sync converges to two blocks.

## Tests

TDD order:

1. Rust rendering test: a bare leaf node is absent from full Markdown.
2. Rust rendering test: an empty parent with a child remains.
3. Two-replica incident test:
   - A creates empty A and syncs to B;
   - B's rendered projection cannot expose A;
   - B creates distinct B;
   - A and B author different entries and cross-import;
   - both replicas converge to two separate blocks;
   - neither entry appears in a `tags::` value.
4. Existing blank-block, materialization, relocation, and sync suites remain green.
5. Client checks confirm no parser assumptions regress; no client behavior fork is introduced.

## Live-data repair

Out of implementation scope. After the code fix is verified, repair the already-corrupted 2026-07-14 daily through the engine in a separate backed-up operation, with explicit user approval. Do not edit only the materialized Markdown because the Loro document would overwrite it.

## Non-goals

- No heuristic splitting of arbitrary merged prose.
- No change to normal same-block concurrent-edit semantics.
- No broad `migrate_in_text` rollout (`tesela-wt5` remains fleet-gated).
- No cleanup/GC of old hidden empty reservations in this patch.
