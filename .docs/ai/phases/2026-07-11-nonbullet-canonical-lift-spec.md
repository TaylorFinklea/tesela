# Non-bullet canonical lift spec

**Bead:** `tesela-myh` · **Decision date:** 2026-07-11 · **Owner tier:** Lead

## Goal

Preserve every imported heading, prose paragraph, unindented fenced block, and
ASCII diagram as visible, editable, synced Tesela content; then unblock
`tesela-ewj.1` and prove the real Logseq import flow in a sandbox product test.

## Product contract

- Loro remains truth; Markdown is a deterministic materialized view.
- Parity is structural, per `decisions.md` 2026-05-28. Original Markdown syntax
  may canonicalize, but no nonblank semantic content may disappear.
- Canonically lift unmodeled Markdown regions into ordinary top-level blocks.
  Do not add a second raw-content CRDT, revive root `content`, or require a
  coordinated fleet activation.
- Query fences become visible fenced-code blocks. Making imported Logseq
  datalog executable is separate scope; the web currently executes `query::`
  blocks, not arbitrary fenced `query` source.

## Architecture

`NoteTree` keeps its existing frontmatter, page-properties, and `FlatBlock`
shape. Replace the lossy body scan with a fence-aware full-coverage scan:

- Existing bullets stay ordinary blocks with their existing bid semantics.
- An otherwise-unmodeled heading or prose paragraph becomes one indent-0
  block. Consecutive prose lines remain one multiline block; a blank line ends
  the region.
- An unindented fenced region becomes one multiline indent-0 block. Its fence
  owns every line through a matching close, including blank lines and lines
  beginning `- `. An unclosed fence owns the rest of the body.
- A continuation belongs to an existing bullet only when it carries at least
  that block's expected continuation indentation. Remove exactly the expected
  prefix, preserving additional indentation inside code/diagrams.
- Blank lines outside a fence are canonical separators and may normalize;
  blank lines inside a fence are content and must survive.

All lifted content then uses the production `blocks` `LoroTree`, stable bids,
`text_seq`, block index, snapshots, relay updates, FFI, web parser, and iOS
parser. Mixed fleets see ordinary blocks; no new reader capability is needed.

### Canonical fenced-block form

A block whose text begins with a fence materializes with a bid-only bullet,
then the entire fence as continuations:

```markdown
- <!-- bid:019... -->
  ```query
  {:find ...}
  ```
```

Parsing a bid-only bullet followed by its first continuation reconstructs the
block text without a synthetic leading newline. Tests must prove the existing
web and iOS block parsers display this as one fenced block and never leak the
bid. Appending a bid comment to the fence opener or closer is forbidden because
it changes the fence info string/closing grammar.

## Safety invariants

1. Every nonblank body line belongs to a page property, ordinary block, or
   lifted region. The scanner reports no silent/unclaimed branch.
2. Parsing then serializing then parsing is structurally idempotent: same
   frontmatter, ordered page properties, indents, and block texts when bids are
   ignored.
3. Fence payload and extra indentation are byte-preserved after removing only
   the canonical list-continuation prefix.
4. `stamp_existing_notes` remains conservative for arbitrary external files;
   ordinary startup does not silently canonicalize them. Explicit engine
   hydration/reseed may canonicalize only when the structural projection is
   proven equal.
5. NoteUpsert reapply is idempotent: stable bids, no duplicate nodes/twins, no
   root `content`, and no snapshot-size body duplication.
6. A stale whole-content NoteUpsert retains the existing absence-is-not-delete
   and tombstone-wins rules.

## Verification corpus

Tests cover: heading before bullets; two prose paragraphs; all-raw page; mixed
raw/bullet/raw; page properties then raw; `query` fence; ASCII fence with blank
lines, extra spaces, and an internal `- ` line; fence inside an existing
bullet; unclosed fence; leading indentation; malformed bid; and no final
newline. Engine tests add cold snapshot reload, two-engine snapshot/delta
exchange, reapply idempotence, adjacent block delete, and snapshot-size guard.

The real-graph sandbox gate copies `~/logseq` without modifying it and proves:

- zero notes skipped for lossy parsing;
- all 19 top-level headings survive as block text;
- all 8 imported query fences survive and display;
- the known `ai-business` diagram and NixOS prose survive;
- restart and unchanged re-import create no loss or duplicate blocks.

## Phase boundaries

1. Core scanner/serializer, semantic-preservation predicate, and golden corpus.
2. Engine hydration, materialization, snapshot, relay, and convergence tests;
   make Rust block indexing plus property/lifecycle classification fence-aware
   so payload lines such as `- literal` and `status:: done` remain inert text.
3. Web/iOS display and edit regressions for lifted heading/prose/fence blocks.
4. `tesela-ewj.1` writer seam plus shared stable note-id helper.
5. In-process server import through the correct active/temporary engine, with
   idempotence and scale integration tests.
6. Real-graph sandbox run, desktop/browser QA, phase report, and durable product
   test artifact for Taylor.

## Out of scope

- Preserving original source syntax byte-for-byte.
- A raw-segment sidecar, mixed-kind tree schema, or root full-body mirror.
- Executing imported fenced Logseq datalog queries.
- Fixing the adjacent sibling-reorder durability gap discovered during review;
  track it separately and include it in regression QA.
