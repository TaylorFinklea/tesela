# Wikilink normalization across Rust, web, and iOS

**Bead:** `tesela-8zd.5` · **Tier:** Lead · **Status:** implementation spec only

## Scope

Make every wikilink target resolve by one canonical, locale-independent key
without changing its visible Markdown. This covers imported Logseq namespaces
and multi-word pages, normal navigation, backlinks, collision reporting, and
Rust/web/iOS conformance.

The current mismatch is concrete:

- `sanitize_filename` in `crates/tesela-core/src/storage/markdown.rs:113`
  converts filesystem-invalid punctuation to `-`, lowercases, and replaces
  spaces.
- The Logseq importer has an independent `safe_name` at
  `crates/tesela-core/src/import_logseq.rs:240-246`; it first maps Logseq
  `___` namespaces to `/`, then only maps `/`, `:`, and spaces.
- Web's `gotoNote` in `web/src/lib/stores/active-pane-nav.svelte.ts:62` passes
  its target through unchanged.
- iOS `wikiAttributed` in
  `app/Tesela-iOS/Sources/Components/BlockText.swift:303-319` lowercases and
  replaces only spaces. `TeselaLink.pageSlug` at `:322-328` then takes the last
  URL path component, silently turning `Parent/Child` into `child`.

`FsNoteStore::get_by_title` is a `WalkDir` scan
(`crates/tesela-core/src/storage/filesystem.rs:182-204`) and is forbidden from
the fallback path. The SQLite `notes.title` column is presently unindexed.

## Binding design

### 1. One canonical target key

The canonical implementation belongs in `tesela-core`, next to the existing
link/slug primitives. Its semantics—not a copied implementation—are the
source of truth. TypeScript and Swift port the exact algorithm and are held to
the shared fixture below.

For a supplied page title, Logseq filename stem, or wikilink target:

1. Trim leading and trailing Unicode whitespace, then normalize Unicode to
   NFC.
2. Interpret Logseq filename namespace spelling before separator handling:
   literal `___` becomes `/`; `%2F` and `%3A` are decoded case-insensitively
   to `/` and `:` respectively. Do not URL-decode any other escape; Markdown
   wikilink text is not a URL.
3. Apply locale-independent Unicode case folding. Device/user locale must not
   affect a link key.
4. Convert each maximal run of Unicode whitespace, `-`, `/`, `\\`, `:`, `*`,
   `?`, `"`, `<`, `>`, `|`, or control characters to one ASCII `-`.
   Other Unicode letters, numbers, and punctuation remain intact.
5. Trim leading and trailing `-`. An empty result is invalid for lookup and is
   recorded as an unresolved target; it is never coerced to a catch-all page.

This intentionally flattens `Parent___Child`, `Parent/Child`, and the
importer's namespace convention to `parent-child`. It also makes `AI/ML`,
`AI: ML`, and `AI ML` all normalize to `ai-ml`. Existing filenames are not
renamed in this bead.

### 2. Resolution order and collision contract

Resolve a target from all eligible candidates, then select exactly one in this
order:

1. **Exact slug:** canonical key equals the stored note slug/id.
2. **Normalized title:** canonical key equals the canonicalized note title.
3. **Alias:** canonical key equals the canonicalized alias.

Within a tier, select the earliest `created_at`; if timestamps tie, select the
lexicographically smallest note id. Higher tiers always win over lower tiers:
a title `Foo` wins over an earlier page whose alias is `Foo`. The resolver must
still report every competing candidate, including candidates shadowed by a
higher tier. Resolution is never silently ambiguous.

The server owns resolution so web and iOS do not independently search stale
client lists. Add an explicit read-only resolution response rather than
changing the shape of the existing `GET /notes/{id}` response. Its contract
includes the supplied target, canonical key, chosen note (or unresolved
status), winning tier, and collision candidates with the selected candidate
identified. Web and iOS must show a non-blocking visible collision notice when
a link follows a tie-broken result; unresolved links remain visibly unresolved.

The alias tier is a locked compatibility contract for `tesela-8zd.6`. This
bead does not import or author aliases; until that child supplies alias data,
the tier is empty in live data. Its collision behavior is nevertheless covered
by the resolver fixture now.

### 3. Indexing and backlinks

Keep `[[display text]]` byte-for-byte in Markdown. At link-index time only,
canonicalize the extracted target before the existing `LinkGraph::update_links`
path writes it. The indexer already calls `extract_wiki_links` followed by
`update_links` in `crates/tesela-core/src/indexer.rs:99-102` and on incremental
updates at `:215-216`; mirror those paths rather than adding a second link
indexer.

Backlink lookup uses the same canonical key. A normal server boot rebuild
already re-extracts and updates links (`crates/tesela-server/src/lib.rs:938-946`),
so existing mosaics gain normalized edges after restart; no separate reindex
bead or destructive migration is required.

Add the required `notes.title COLLATE NOCASE` SQLite index in
`crates/tesela-core/src/db/schema.rs` and query it for title candidates rather
than walking files. If a canonical-title key must be materialized to represent
the full separator-normalization contract, maintain and index that projection
at the same SQLite write seam; it is a cache of the core normalizer, never a
second algorithm. Do not route title lookup through `FsNoteStore::get_by_title`.
The indexed queries must return all matches so the deterministic tie-break and
collision report remain possible.

### 4. Import planning and round trip

The importer uses the core normalizer for its generated slug, but never rewrites
wikilink display text in converted note bodies. During import planning it must
surface all slug-normalization collisions before apply: raw source names,
canonical key, and every conflicting source. A slug collision blocks those
plan items until the user renames or skips a source; apply must not select or
silently overwrite one imported page with another.

A write → parse → index → render round trip preserves `[[Parent/Child]]` in
Markdown while navigation and backlinks use `parent-child`. This is a durable
file-format invariant, not a display-only shortcut.

## Shared three-engine fixture

Create one JSON fixture under `crates/tesela-core/tests/fixtures/`, modeled on
the existing Rust/web/iOS query-conformance consumers. Each consumer must call
its real production normalization/resolution adapter, not duplicate fixture
logic:

- Rust: a `tesela-core` integration test beside the existing fixture consumers.
- Web: a Node test under `web/tests/unit/` importing the production navigation
  normalization/resolution adapter.
- iOS: an XCTest under `app/Tesela-iOS/Tests/` calling the production link
  adapter used by `BlockText`/`TeselaLink`.

The fixture contract includes raw input, expected canonical key, candidate
notes (slug/title/alias/created timestamp/id), selected result, all collision
candidates, and whether source Markdown must remain unchanged. Required cases:

- `[[Parent/Child]]` navigates to and backlinks from `parent-child.md`.
- `Parent___Child`, `%2F`, and `%3A` use the same namespace flattening.
- `AI/ML`, `AI: ML`, and `AI ML` collide at `ai-ml` and are surfaced.
- A case-variant target resolves identically without locale dependence.
- Exact slug beats title; title beats alias; title-vs-title and alias-vs-alias
  choose earliest creation then note id; title-vs-alias reports the shadowed
  alias.
- Empty/invalid-normalization targets remain unresolved.
- The persisted `[[Parent/Child]]` text is unchanged while its stored backlink
  key is `parent-child`.

## Sequencing

### 1. Lock the canonical contract and ports

**Work:** Add the fixture and the core normalizer; replace the four divergent
normalization paths with ports of that contract. Inspect the verified current
sites above and the existing query/inline-span fixture consumers before editing.
Do not write a fresh parser per client.

**Acceptance:** The required fixture matrix passes through the real Rust, web,
and iOS adapters; `TeselaLink.pageSlug` retains all namespace components rather
than `path.last`.

**Verify:** `cargo test -p tesela-core`; `pnpm --dir web test:unit`; `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`.

### 2. Make indexed resolution authoritative

**Work:** Add the SQLite title index/migration and server-owned resolver,
including candidate collection, priority, deterministic tie-break, and the
collision response. Mirror the existing schema migration and note-route idioms;
do not change `GET /notes/{id}`'s response shape. Normalized-title lookup must
be a database query, never a filesystem walk.

**Acceptance:** Resolution returns the tiered winner and all collision context;
`FsNoteStore::get_by_title` is not called by the path; a title lookup uses the
new index; a normalized `Parent/Child` target resolves to `parent-child`.

**Verify:** `cargo test -p tesela-core -p tesela-server`; `cargo clippy -p tesela-core -p tesela-server -- -D warnings`.

### 3. Normalize link edges and import preflight

**Work:** Canonicalize only extracted/indexed targets on both full rebuild and
incremental update paths. Update the Logseq import planner to use the shared
normalizer and produce actionable collision warnings before apply, preserving
body display text.

**Acceptance:** A fresh boot rebuild turns existing raw namespace links into
backlink edges; import planning exposes every normalized filename collision
and blocks its unsafe apply; converted Markdown still contains the original
wikilink spelling.

**Verify:** `cargo test -p tesela-core -p tesela-server`.

### 4. Adopt the resolver in web and iOS navigation

**Work:** Route web `gotoNote` and iOS wikilink taps through the server
resolution contract. Preserve existing successful navigation behavior, make
unresolved state visible, and surface collision notices without blocking the
selected page. Mirror the existing web navigation and iOS `OpenURLAction`
patterns; do not introduce a second client-side candidate search.

**Acceptance:** `[[Parent/Child]]` opens the full parent-child page on web and
iOS; a collision is observable to the user; an unresolved link does not
silently navigate to a wrong page.

**Verify:** `pnpm --dir web check`; `pnpm --dir web test:unit`; `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`.

### 5. Run the three-engine and restart regression gates

**Work:** Run the shared fixture from all consumers and add a server integration
case that starts from stored raw namespace links, rebuilds the index, and reads
backlinks through the normal route.

**Acceptance:** All required fixture cases pass on all three engines and the
server regression proves raw display text plus normalized navigation/backlinks.

**Verify:** `cargo test -p tesela-sync -p tesela-server && pnpm --dir web run check && pnpm --dir web test && xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`.

## Out of scope

- Implementing alias import/authoring/search UI (`tesela-8zd.6` owns that
  data-production work).
- Renaming existing Markdown files or rewriting visible wikilink text.
- Fuzzy matching, search ranking, redirect histories, or automatic
  collision-resolution UI.
- Cross-mosaic links and external URL normalization.
- A separate manual reindex command or data migration for existing links.
