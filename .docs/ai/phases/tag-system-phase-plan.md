# Tag system — phase plan

Implements [`.docs/designs/2026-05-17-tag-system.md`](../../designs/2026-05-17-tag-system.md).
Each phase ends with a commit. Phases are sized to one Build session each.

The existing tag surface (per inventory) is more substantial than the spec
assumes: `GET /tags`, `ensure_tag_pages` auto-creation, `block-tags.ts`
toggle plumbing, `cm-decorations.ts` `#tag` regex (currently hides inline
tokens), and a `TagsSurface` sidebar exist. This plan extends them rather
than starting from zero.

The 16 logical phases in the spec collapse into **5 execution bundles** so
each bundle is a coherent commit.

---

## Bundle A — Tag entity foundation (Phases 1-3)

### Phase 1 — Type registry + Reference: tag

- Normalize backend's auto-created tag pages from `type: "Tag"` to
  `type: tag` (lowercase) per spec.
  Files: `crates/tesela-server/src/routes/notes.rs:969-1015`.
- Migration sweep: on startup, rewrite any existing `type: "Tag"` pages
  to lowercase. Idempotent.
- Register a `tag` page-type renderer (skeleton — renders the page like a
  note for now, gets a "tag" chip in the buffer header).
  Files: `web/src/lib/renderers/page/index.ts`, new
  `web/src/lib/renderers/page/tag-page.svelte`.
- Wire `Reference: tag` resolution: clicking a `#tag` in the editor opens
  `tags/<slug>.md` as a page buffer.
  Files: `web/src/lib/v5/active-pane-nav.svelte.ts`,
  `web/src/lib/components/v4/active-pane.ts`.

### Phase 2 — Tag entity storage (slug rules + parent frontmatter)

- Tag files at `tags/<slug>.md` (flat). Update `ensure_tag_pages` to
  emit files under `tags/` directory if not already there.
- Add `parent: <slug>` frontmatter field (optional). Update
  `NoteMetadata` ts-rs binding.
- Slug collision auto-numbering: helper that picks the next free slug.
  Files: `crates/tesela-server/src/routes/notes.rs`.
- `rename-slug` verb (palette).
  Files: `web/src/lib/v4/commands.ts`,
  `web/src/lib/api-client.ts` (`renameTagSlug`).

### Phase 3 — Path syntax resolution + cascade-create

- Backend handler: `POST /tags/resolve` — input `{ path: string }`,
  output `{ slug, created: bool, cascade_created: string[] }`. Walks the
  path, finds an existing chain or cascade-creates.
- Frontend: when autocomplete picks a path-form suggestion, call resolve
  to materialize the chain, then insert the bare slug into the source.

**Bundle A exit criteria:**
- All existing tag pages are `type: tag` (lowercase).
- Clicking `#fella` in an editor opens the tag's page.
- `:rename-slug` cycles a slug.
- Path-form autocomplete cascades.

---

## Bundle B — Block tag tokens (Phases 4-6)

### Phase 4 — Position-aware block parser

- Update `crates/tesela-core/src/block.rs:parse_blocks` to record token
  positions: each `#tag` gets `(slug, start_offset, is_trailing)`. The
  trailing-cluster regex matches `(\s*#[A-Za-z0-9_/-]+)+\s*$` on the
  block's last paragraph.
- `ParsedBlock` gains `trailing_tags: Vec<String>` separate from
  `inline_tags: Vec<String>`. `tags: Vec<String>` becomes their union for
  back-compat.

### Phase 5 — Inline vs chip rendering

- `web/src/lib/cm-decorations.ts`: stop hiding inline `#tag` (currently
  replaced by `Decoration.replace` widget). Render them as styled
  clickable spans (mark decoration).
- Detect the trailing cluster in the editor: re-run the same regex per
  block line; replace the trailing-cluster range with a chip widget
  decoration that renders one `<button class="tag-chip">` per token.
- Clicking a chip or inline tag opens the tag's page (via
  `Reference: tag`).

### Phase 6 — Cmd+Enter promote/demote

- Editor keybind. On inline `#tag`: slice the token, append to end of
  block.
- On trailing chip: slice from trailing cluster, splice into cursor
  position.
- Pure text rewrite — no sidecar changes.

**Bundle B exit criteria:**
- Type `#foo bar #baz` in a block; `#foo` renders inline, `#baz` renders
  as a trailing chip.
- Cmd+Enter on `#foo` moves it to the end (becomes chip).
- Cmd+Enter on the `#baz` chip moves it back to cursor (becomes inline).
- Clicks navigate to the tag page.

---

## Bundle C — Autocomplete (Phases 7-8)

### Phase 7 — `[[` autocomplete with disambiguation

- Existing `[[` autocomplete needs disambiguation when same-name page
  and tag exist.
  Files: the existing wiki-link autocomplete module (find via
  `grep -rn "\\[\\[" web/src/lib/cm-blocks.ts` etc.).
- Result rows show: icon (page/tag), name, type-chip, parent-path (for
  tags).
- Sort: most-recently-edited.

### Phase 8 — `#` autocomplete (tag-only)

- New autocomplete trigger on `#`. Matches only `type: tag` pages.
- Empty match → "Create new tag" action that materializes a new tag
  page via Phase 3's resolve endpoint.
- Ambiguous bare name (`#cardinal` with two cardinals) → list with
  parent-path subtitles.
- Sort: most-recently-used-as-a-tag (tracked client-side via a small
  LRU in `web/src/lib/state/recent-tags.svelte.ts`).

**Bundle C exit criteria:**
- Typing `[[fella` shows both `fella` (note) and `fella` (tag) with
  disambiguation chips.
- Typing `#fell` shows tags only.
- Typing `#new-tag-name` then Enter creates the tag and inserts.

---

## Bundle D — Tag-page renderer + derived renderers (Phases 9-11)

### Phase 9 — Tag-page hybrid renderer

- `web/src/lib/renderers/page/tag-page.svelte` becomes the composite:
  description outliner on top, embedded `instances-of-tag` table on
  bottom.
- Outliner section collapsed by default if empty; auto-expanded if
  content exists.
- Keybind to force collapse/expand.

### Phase 10 — `instances-of-tag` derived renderer

- New file: `web/src/lib/renderers/derived/instances-of-tag.svelte`.
- Backend handler: `GET /tags/{slug}/instances` →
  `{ page_instances: [...], block_instances: [...] }`.
- Reuse the existing `query` results table component.

### Phase 11 — `backlinks-of-tag` derived renderer

- New file: `web/src/lib/renderers/derived/backlinks-of-tag.svelte`.
- Backend handler: `GET /tags/{slug}/backlinks` — same shape as page
  backlinks.
- Add to Peek's default rotation when the focused buffer is a tag page.

**Bundle D exit criteria:**
- Opening a tag page shows description on top, instances table on
  bottom.
- A new derived buffer with the tag as Reference shows backlinks /
  instances per the renderer.
- Peek on a tag page cycles through the tag-flavored renderers.

---

## Bundle E — Operations (Phases 12-16)

### Phase 12 — Frontmatter tag ops

- Page-level `tags: [...]` frontmatter add / remove from a UI surface
  (likely the page header chips on a non-tag page).
- `#` autocomplete in the chip-edit popover.

### Phase 13 — Cascade rename on parent change

- When a tag's `parent:` changes, rewrite path-form references in
  markdown workspace-wide. Reuse the page-rename rewrite plumbing.
  Files: `crates/tesela-server/src/routes/notes.rs:update_links` or
  equivalent.

### Phase 14 — Convert verbs

- `:convert-to-tag` palette verb on a `type: note` page: rewrites
  frontmatter `type`.
- `:convert-to-note` palette verb on a `type: tag` page: same, inverse.

### Phase 15 — Deletion UX

- `:delete-tag` palette verb on a tag page.
- Shows the four counts (refs, page-instances, block-instances,
  children).
- Two choice prompts (refs handling, children handling).
- Executes the chosen plan.

### Phase 16 — Query DSL extensions

- Extend the query parser in `crates/tesela-server/src/routes/search_query.rs`
  to accept `tag:fella`, `pagetag:fella`, `blocktag:fella` filters.
- Wire to `instances-of-tag` so the table can be driven by a query.

**Bundle E exit criteria:**
- All five operations work end-to-end.
- Query DSL filters work for tag membership.
