# Wikilink normalization — Loro-index resolution across Rust, web, and iOS

**Bead:** `tesela-8zd.5` · **Tier:** Lead · **Status:** revised implementation spec

## Review disposition

This revision adopts every blocker and major finding in the 2026-07-10 Sol
adversarial review. There are no contested findings.

- Resolution is a `tesela-sync` capability over the always-resident Loro index,
  exported through FFI; the server route is only the web adapter.
- Tie-breaking uses a durable creation-order field in each note's Loro root and
  index entry, never filesystem birth time.
- SQLite receives explicit canonical-key columns; `COLLATE NOCASE` is not used
  as a Unicode normalizer.
- The Loro index derives normalized links, all web wikilink entry points share
  one resolver/open adapter, and import collisions are serialized blockers.

## Verified baseline

The current tree has four divergent paths: core's `sanitize_filename`, the
Logseq importer's `safe_name`, web's direct `gotoNote`, and iOS
`BlockText.wikiAttributed`. The current Loro index derives `links` with raw
`extract_wiki_links` targets in
`crates/tesela-sync/src/engine/loro_engine/index.rs`; its FFI projection is
`IndexEntryRecord` in `crates/tesela-sync-ffi/src/lib.rs`.

The server still maintains the rebuildable SQLite link cache:
`SqliteIndex::update_links` stores `links.target` verbatim and
`get_backlinks` matches it verbatim. The existing `notes.title` field has no
canonical-key projection. `FsNoteStore::get_by_title` remains forbidden here:
it walks files and is not a resolver fallback.

The Graphite shell has two relevant raw navigation paths: the wiki click paths
in `BlockEditor.svelte` call `gotoNote(target)`, and `GrPage.svelte`'s
`openRef` opens a target directly with `openPageInFocused`. On iOS,
`TeselaLink.pageSlug` takes only the final URL path component, losing the
parent portion of `[[Parent/Child]]`.

## Decision: one versioned normalization contract

### Canonical key v1

`tesela-core` owns the normative `wikilink-key-v1` contract. It accepts raw
wikilink target text, title text, imported filename stems, and aliases. It
returns either a non-empty canonical key or `invalid`; it never guesses a
page. Markdown display text is never rewritten.

The exact pipeline is:

1. Trim Unicode whitespace and NFC-normalize.
2. Decode only case-insensitive `%2F` and `%3A`; translate Logseq filename
   spelling `___` to `/`. No other percent escape is decoded.
3. Apply Unicode **Default Case Folding**, non-Turkic, from a pinned Unicode
   CaseFolding data version. It is not locale-sensitive lowercasing.
4. Replace each maximal run of Unicode whitespace, `-`, `/`, `\\`, `:`, `*`,
   `?`, `"`, `<`, `>`, `|`, or control characters with one ASCII `-`.
5. Trim leading and trailing `-`. The empty result is `invalid`.

The implementation must generate the Rust, TypeScript, and Swift case-fold
lookup tables from one checked-in Unicode data revision. Platform
`lowercased()`, `toLowerCase()`, SQLite `NOCASE`, and device locale are not an
acceptable substitute. The shared fixture is both the conformance source and
its generation input; ports call their production normalizer.

Required vectors include:

| Raw input | Expected v1 key / rule |
| --- | --- |
| `Parent/Child`, `Parent___Child`, `Parent%2fChild`, `Parent%3AChild` | `parent-child` |
| `Cafe\u{301}` | `café` after NFC |
| `Straße`, `STRASSE` | `strasse` |
| `ΟΣ`, `ος` | `οσ` (final sigma folds to sigma) |
| `I`, `İ`, `ı` | `i`, `i\u{307}`, `ı`; no Turkish-locale special case |
| `\u{2002}Parent\u{00A0}/\tChild` | `parent-child` |
| `Parent///--- ::: Child` | `parent-child` |
| `---` | invalid |

### Durable Loro resolution data

The current index is a derived, always-resident Loro document with per-note
`title`, `slug`, `tags`, and `links`; it is therefore the correct authority
for relay-only iOS resolution. Bump its schema and rebuild it from per-note
roots to add these fields to every index entry:

| Field | Meaning |
| --- | --- |
| `slug_key` | `wikilink-key-v1(slug)` |
| `title_key` | `wikilink-key-v1(title)` |
| `links_key` | newline-delimited normalized outbound targets; raw Markdown remains only in the note document |
| `creation_order` | immutable durable tuple `(first_creation_millis, note_id_hex)` |

`creation_order` is written once into the per-note Loro `root` map on its first
creation and copied into the index entry. Subsequent `NoteUpsert`s may update
title/slug/content but must not rewrite it. It is sourced from the first
creation payload, not a filesystem timestamp. A legacy note with no root field
receives the deterministic migration sentinel `(0, note_id_hex)` during index
schema rebuild; this makes legacy collisions deterministic without pretending
the filesystem can recover their original creation time. The tuple's note-id
suffix is the total-order tie breaker.

`extract_index_metadata` must normalize wiki targets before it writes
`links_key`; full rebuilds, relay imports, and iOS local index reads therefore
agree. This closes the relay-only backlinks/navigation gap rather than merely
repairing the server SQL cache.

Add a `tesela-sync` resolution record and operation that consume this Loro
index, returning:

- supplied target and canonical key;
- `resolved` or `invalid`/`unavailable` result;
- selected note id and slug when resolved;
- winning tier; and
- every candidate, including candidates shadowed by a higher tier.

Candidate tiers are fixed: exact **stored slug key**, then normalized title,
then alias. Alias candidates remain empty until `tesela-8zd.6` produces them,
but the resolver record and fixture include the tier now. Within one tier,
sort `(creation_order, note_id)` ascending. A collision is always returned;
choosing the first candidate is deterministic but never silent.

Expose that operation through `tesela-sync-ffi` as a Swift-friendly resolution
record. iOS obtains it from its local Loro engine, not from `tesela-server`, so
navigation still works while the Mac is off. The server adds a read-only route
that delegates to exactly this engine operation for web; it owns no separate
candidate search or tie-break implementation.

### Rebuildable SQL projection and backlinks

SQLite remains a cache for server/web backlinks, not the resolution authority.
Its migration adds explicit values populated by the core normalizer in the
same note upsert transaction:

- `notes.slug_key`, `notes.title_key`, and `notes.creation_order` with indexes
  on the two key fields; and
- `links.target_key`, indexed for backlink lookup, while `links.target` keeps
  the raw target for display/API compatibility.

`LinkGraph::update_links` computes `target_key` from the core normalizer; all
existing server write paths already funnel through that method. Backlinks query
by the target page's `slug_key`. A boot rebuild regenerates the cache from
Markdown, but it must retain raw link display text. SQLite `NOCASE` is not part
of the lookup algorithm.

## Client adapters

### Web

Add one production `resolveAndOpenWikilink(rawTarget)` adapter. It calls the
server's engine-backed resolver, shows a non-blocking collision notice when
candidates exist, and only then passes the chosen note id to the buffer/route
navigation primitive. It is the only web path allowed to open a wikilink.

Replace both existing wiki click paths in `BlockEditor.svelte` and the
link/backlink `GrPage.openRef` path with that adapter. `gotoNote` remains the
low-level direct navigation primitive for callers that already have a canonical
note id; no raw wikilink target may bypass resolution through it. Normal
clicks on unavailable/invalid links leave the editor in place and show an
unobtrusive error.

### iOS

`wikiAttributed` retains the full raw target as an opaque link intent; it must
not encode namespace separators as URL path components and must not select
`URL.pathComponents.last`. The OpenURL handling path calls the FFI resolver,
then opens the returned note id. Thus the `Parent/Child` parent segment cannot
be discarded. Collision and unavailable states are visible but do not make the
link silently open a different note.

## Import-plan safety and round trip

Extend the serde `ImportPlan` returned by `POST /imports/logseq/plan` with a
serializable normalization-conflict collection. Each entry contains the raw
source names, source-relative paths, proposed normalized key, and every
conflicting item. Each conflict is `blocking: true` until the supplied
`ApplyDecisions` explicitly renames or skips all but one candidate. Apply
rejects an unresolved blocking conflict; it cannot pick the first item or
silently overwrite it. The existing plan/preview/apply flow is the surface to
extend, not a second import API.

The importer consumes `wikilink-key-v1` for generated target slugs and retains
`[[Parent/Child]]` byte-for-byte in converted note bodies. The durable
round-trip assertion is: parse → Loro index/SQL cache → navigation/backlink
preserves that source Markdown while resolving and backlinking via
`parent-child`.

## Tests and acceptance

One shared fixture must drive Rust core/sync, the real web adapter, and the
FFI/iOS adapter. It covers every normalization vector above plus:

- `[[Parent/Child]]` resolves and backlinks to `parent-child.md` after an
  index rebuild on Rust, web, and iOS;
- exact slug beats title; title beats alias; every losing candidate is exposed;
- title/title and alias/alias ties use durable `creation_order`, then note id;
- a case variant and all separator variants resolve identically;
- an alias/title collision is represented even before alias authoring ships;
- Loro relay import produces the same `links_key` as local creation; and
- old index snapshots missing the new fields rebuild deterministically.

Add server integration coverage for raw namespace Markdown → normalized SQL
backlinks; add import-plan API coverage proving unresolved normalized-name
collisions cannot apply; add a Graphite test proving every wiki/refcard entry
uses the shared adapter; and add iOS coverage for the retained namespace.

**Verify:**

```bash
cargo test -p tesela-core -p tesela-sync -p tesela-server
pnpm --dir web check
pnpm --dir web test:unit
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'
```

## Sequencing

1. Define v1 normalizer, Unicode-generated ports, fixture, and SQL migration.
2. Add durable root/index fields, normalized link extraction, engine resolver,
   and FFI records; prove two engines resolve the same collision after sync.
3. Make server resolution/backlinks an adapter/cache projection of the engine.
4. Make import plans block normalization collisions and preserve raw links.
5. Route every web/iOS wikilink path through the resolver and run the complete
   fixture plus platform integration gates.

## Out of scope

- Alias authoring/import UI (`tesela-8zd.6`), fuzzy search, redirects, file
  renames, and cross-mosaic/external URL links.
- Treating a collision notice as user-driven collision resolution; import
  collisions must be resolved before apply.
