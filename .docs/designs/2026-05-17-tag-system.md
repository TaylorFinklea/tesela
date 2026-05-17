# Tag system — Tesela web client

Builds on Prism v5 chrome ([`2026-05-15-prism-v5-chrome.md`](./2026-05-15-prism-v5-chrome.md)).
The v5 `Reference` union already names `{ kind: "tag", value: string }` as a
first-class type; this spec fills in tag behavior, storage, rendering, and
editor UX.

Tesela context unchanged: local-first markdown, every artifact is a page on
disk with frontmatter (`type`, `tags`, `properties`), SvelteKit + Svelte 5
runes + TanStack Query.

## Conceptual model

> **Invariant** — A tag is a page with `type: tag`. Distinct from any page
> with the same name but a different `type:`.

A note named `fella` (`type: note`) and a tag named `fella` (`type: tag`)
coexist as distinct pages. The `Reference` union discriminates by `kind`, so
the editor and renderers can tell which is which even when names collide.

> **Invariant** — `[[link]]` references any entity. `#tag` references a tag
> *and* classifies the surrounding block (or page) by that tag.

Classification is strictly more than reference: every `#tag` is a `[[link]]`
plus a membership claim ("this block is a `fella`"). The two syntaxes are
*not* interchangeable.

**Enforcement points:**
- The block parser (`crates/tesela-core/src/block.rs`) extracts `#tag`
  tokens into `ParsedBlock.tags`; that field is the membership claim. A
  `[[fella]]` link with no `#fella` does not classify the block.
- The frontend reference resolver (`Reference: tag`) routes to the tag's
  page, the same way `Reference: page` routes to a page's page.

## Editor UX

### Inline tag insertion

> **Invariant** — Typing `#`<query> inside a block opens the tag
> autocomplete. Selecting a result inserts `#<canonical-form>` at the
> cursor, with no chrome the markdown can't represent.

- `#` autocomplete matches **only `type: tag` pages**, sorted by
  most-recently-used-as-a-tag.
- Empty-match shows "Create new tag" which cascade-creates if a path is
  typed (`#nature/birds/cardinal` cascade-creates `nature` then
  `nature/birds` then `cardinal`).
- Ambiguous bare names (`#cardinal` when two `cardinal` tags exist) list
  each candidate with its parent-path as subtitle.

### Wiki-link insertion

> **Invariant** — `[[` autocomplete matches all pages including tags,
> sorted by most-recently-edited, with disambiguation (icon + type-chip)
> when same-name entities exist.

- Same-name disambiguation: when both `[[fella]]` (page) and `[[fella]]`
  (tag) match the query, the autocomplete shows both rows with icons +
  `note` / `tag` type-chips. User picks.

### Promote / demote (Cmd+Enter)

> **Invariant** — `#tag` chip-vs-inline rendering is determined by token
> position in the source text, not by sidecar metadata.

- **Trailing-cluster rule**: one or more `#tag` tokens at the end of a
  block's text content, separated only by whitespace, render as **chips**
  at the end of the block.
- All other `#tag` tokens render **inline** as styled tokens.
- `Cmd+Enter` on an inline `#tag` cuts the token and appends it to the end
  of the block (it becomes a chip).
- `Cmd+Enter` on a trailing chip moves it back to the cursor position
  inside the block content (it becomes inline).
- The operation is purely text-level: read source, slice the token, splice
  it elsewhere, write back. No sidecar to keep in sync.

**Why position-as-data**: markdown stays portable. External tools (Obsidian,
Logseq, plain `grep`) see `#tag` and know it's a tag. They don't render
chips, but they render *something*.

**Enforcement point**: the block renderer in `web/src/lib/cm-decorations.ts`
and the block parser in `crates/tesela-core/src/block.rs` agree on the
trailing-cluster regex. Both must update together if the rule changes.

## Storage

### Block-level tag storage

> **Invariant** — `#tag` tokens in a block's content are the block's tags.
> No sidecar field.

- Parser (`crates/tesela-core/src/block.rs:parse_blocks`) extracts tokens
  into `ParsedBlock.tags`. Already implemented and indexed.
- Block-level tags participate in `instances-of-tag` aggregation.
- The legacy `tags::` property line (Phase 10.5 work) is kept as a
  **read-only back-compat path**: parser reads it into `ParsedBlock.tags`
  alongside inline `#tag` tokens, but new writes append `#tag` to block
  content rather than touching `tags::`.

### Page-level tag storage

> **Invariant** — Page-level tags live in frontmatter `tags: [...]`, an
> array of strings. Independent of block-level tags.

Unchanged from v5; already wired (`NoteMetadata.tags`, `list_tags()`).

### Tag entity storage

> **Invariant** — Each tag is a markdown file at `tags/<slug>.md`. The
> slug is path-flat: no directory hierarchy on disk.

- Frontmatter:
  - `type: tag` (lowercase; the current `type: "Tag"` capitalization is
    normalized to lowercase as part of Phase 1).
  - `parent: <slug>` — optional; absent means a top-level tag.
  - Standard fields (`tags`, `properties`, etc.) — a tag can be tagged.
- **Slug collision rule**: creating a tag named `cardinal` when
  `tags/cardinal.md` already exists picks the next free auto-numbered slug
  (`cardinal-2.md`, `cardinal-3.md`, …). The slug is internal; the
  display name is the **leaf** of the parent chain. A `rename-slug` verb
  in the palette lets the user pick a stable disambiguator (e.g.,
  `cardinal-religion`).

**Why flat on disk**: stable slug across hierarchy moves. Moving `cardinal`
from under `nature/birds` to under `religion/symbols` doesn't rename the
file; it only changes `parent:` frontmatter on the tag and rewrites
path-form references in markdown.

**Enforcement point**: tag-creation routes
(`crates/tesela-server/src/routes/notes.rs:ensure_tag_pages` and the new
`create_tag` handler) compute the free slug via collision check, never via
parent-chain interpolation.

## Hierarchy and path resolution

### Path-form references

> **Invariant** — In markdown, a tag reference is *either* a bare slug
> (`#cardinal`) *or* a path of slugs (`#nature/birds/cardinal`). The
> rightmost segment is the tag's slug; preceding segments are the parent
> chain.

- Bare references resolve at read time:
  - Exactly one match → resolves to that tag.
  - Multiple matches → autocomplete disambiguates at *write* time
    (the user picks); at read time, the renderer shows a "?" badge and
    a click-to-disambiguate menu.
  - No match → autocomplete offers "Create new tag" at write time; at
    read time, renders as an unresolved-tag widget.

### Cascade-create

> **Invariant** — Resolving a path-form reference that doesn't fully
> match the existing hierarchy creates the missing ancestors, top-down,
> as empty tags with `parent:` set to the previous segment's slug.

Triggered only at autocomplete commit (user explicitly chose to insert
a path-form), never at read time.

### Parent rename

> **Invariant** — Changing a tag's `parent:` frontmatter triggers a
> workspace-wide rewrite of path-form references that include this tag's
> slug. The tag's own file location does not change.

Reuses the page-rename rewrite plumbing (already exists for `[[link]]`
renames). The slug stays put; only the path the user sees and types in
new references changes.

## Derived renderers

### `backlinks-of-tag`

> **Invariant** — Lists every block whose content contains the focused
> tag, with reading context. Equivalent to `backlinks-of-page` but for
> tags.

- Accepts `Reference: tag`.
- Used in derived buffer panes and in Peek.

### `instances-of-tag`

> **Invariant** — Lists every entity that is an instance of the focused
> tag, where:
> - **page-level instance** = the page has the tag in its frontmatter
>   `tags:`.
> - **block-level instance** = a block on the page has the tag in its
>   parsed `tags` (either inline `#tag` or legacy `tags::`).

- Renders as a table (same component as the `query` page-type's results
  table).
- Visual marker per row: page-icon + `page` kind label, or block-icon +
  truncated block content + clickable parent-page name + `block` kind
  label.
- A page that is both page-level *and* has block-level instances renders
  as one page-level row plus N block-level rows. Not deduplicated.

Both renderers are host-agnostic: they mount inside derived buffer panes
and inside Peek without host knowledge, per the v5 protocol.

## Tag-page renderer (`type: tag`)

> **Invariant** — A `type: tag` page renders as a composite of two
> sections: an editable description (outliner) on top, an embedded
> `instances-of-tag` table on the bottom. Both share the page's height.

- Top section: standard block outliner. Collapsed by default if empty,
  auto-expanded when content exists. A toggle keybind forces expand /
  collapse.
- Bottom section: mounts the `instances-of-tag` derived renderer with the
  current tag as `Reference`. Always visible.
- The page renderer is a host for the derived renderer (no protocol
  extension needed — `DerivedRenderer` is host-agnostic, and the page
  renderer behaves as any other host).

**Enforcement point**: the page-renderer registry route for `tag` (in
`web/src/lib/renderers/page/index.ts`) mounts the composite component;
the composite imports the derived registry's `mount("instances-of-tag",
ref)` to fetch the sub-renderer.

## Convert verbs

> **Invariant** — `convert-to-tag` and `convert-to-note` are inverses: a
> tag converted to a note and back yields the same tag, with its content
> and frontmatter intact.

- `convert-to-tag` on a `type: note` page: rewrites `type: note` →
  `type: tag`. Existing content becomes the description.
- `convert-to-note` on a `type: tag` page: rewrites `type: tag` →
  `type: note`. Description content stays. `parent:` and `tags:`
  frontmatter are preserved (so the inverse round-trips).
- References (`[[name]]`, `#name`) remain valid; the page identity didn't
  change, only its kind.

## Deletion UX

> **Invariant** — Deleting a tag is never silently destructive. The user
> sees counts and chooses an explicit handling for each affected
> dimension.

Counts shown:
- N path-form references in markdown.
- N pages with this tag in their `tags:` frontmatter.
- N blocks with `#tag` token (or `tags::` row) for this tag.
- N child tags (tags whose `parent:` is this tag).

User choices:
- **References**: leave as broken tokens, *or* auto-clean (strip `#tag`
  from markdown).
- **Child tags**: orphan (set their `parent:` to null), *or*
  cascade-delete (recursive prompt), *or* reparent to this tag's parent.

Default: non-destructive (leave references, orphan children).

## Slug rules

- Auto-numbered (`cardinal-2.md`) on collision.
- `rename-slug` verb in the palette: prompts for the new slug, validates
  uniqueness, rewrites `parent:` references on children, rewrites
  path-form references in markdown.

## Out of scope (deferred)

- **Class-property schemas** (Logseq-style: a tag defines properties its
  instances should have). Meaningful design exercise of its own.
- **Tag aliases** (multiple names for the same tag entity).
- **Per-tag visibility / scoped tag namespaces** beyond hierarchy.
- **Tag merging** (combining two tag entities into one).

## Rejected alternatives

- **Tag = same entity as a page (Logseq DB style)**. Rejected: a note
  named `fella` and a tag named `fella` are different entities; Tesela
  uses distinct entities with disambiguation at the type level.
- **Tag = pure label (Obsidian style)**. Rejected: tags have content
  (description) and hierarchy; a label is too thin.
- **Marker syntax `{#tag}` for chip rendering**. Rejected: markdown
  portability matters, and position is data — the trailing-cluster rule
  expresses chip-vs-inline without leaving the standard `#tag` token.
- **Flat hierarchy**. Rejected: users want grouping; the path syntax +
  frontmatter parent expresses hierarchy without forcing path-as-slug.
- **Pure path-as-slug** (`tags/nature/birds/cardinal.md`). Rejected:
  moving a parent renames every descendant's file path and every
  path-form reference at once. Flat slug + `parent:` frontmatter keeps
  the tag's identity stable across hierarchy moves.
- **Sidecar field for chip vs inline**. Rejected: another shape to keep
  in sync; markdown stops being source-of-truth. The trailing-cluster
  rule reads position from the source.
- **Block-level tags in `tags::` property only**. Rejected (rolling back
  the Phase 10.5 decision): `tags::` separates the tag from the surface
  where the user thinks about it (the prose). Inline `#tag` keeps
  classification next to its evidence.
