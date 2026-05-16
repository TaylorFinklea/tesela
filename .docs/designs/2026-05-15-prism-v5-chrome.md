# Prism v5 chrome — Tesela web client

## Context

Prism v4 introduced a tmux/Zellij-shaped chrome with five pane kinds (editor,
widget, context, graph, dashboard). After several days of dogfooding, the kind
grab-bag is conceptually muddled: panes mix editing, query rendering,
metadata-following, and full ambient surfaces, with no shared invariant
between them. The mental model fails the first-principles test: what does it
mean to "open a thing as a pane?"

Prism v5 replaces the five-kind grab-bag with a tightly-typed **three-kind
buffer set**, removes widget / dashboard / graph as pane kinds (they become
ambient buffers, or move into overlays), and adds a dedicated, collapsible
left navigation sidebar in the Neovim NvimTree idiom. The cutover is
**rip-and-replace**: same files, no parallel `/v5` route, no feature flag.
Git main is the rollback path.

The work targets the web client only. Rust core, sync substrate, and the
SwiftUI client are out of scope.

## Regions

```
┌──────────────────────────────────────────────────────────────┐
│ brand · tabs · command bar · icons                top bar    │
├──┬─────────────────────────────────────────────────┬─────────┤
│☰ │                                                 │         │
│⚲ │                                                 │         │
│⏲ │           main pane tree                        │         │
│☆ │           binary splits of buffers              │         │
│♬ │                                                 │         │
│  │                                                 │         │
│◄ │                                                 │         │
├──┴─────────────────────────────────────────────────┴─────────┤
│ status line                                                  │
└──────────────────────────────────────────────────────────────┘
   ^
   left sidebar
   switchable surfaces, collapsible via ◄
```

### Top bar

- Brand mark + workspace name.
- Tab strip (one tab = one pane tree).
- Command bar (button that opens ⌘K Command Station; visible kbd hint).
- Icons: graph (⌘G), settings (⌘,).

### Left sidebar

- Collapsible. Icon strip on the far-left edge persists when collapsed.
- **Switchable surfaces**: one content area visible at a time. Icons in the
  strip swap which surface is mounted. Chord-driven from the keymap,
  click-driven from the strip.
- Initial surfaces, top-to-bottom in the icon strip:
  1. **Notes tree** (`☰`)
  2. **Search** (`⚲`)
  3. **Recent** (`⏲`)
  4. **Pinned** (`☆`)
  5. **Tags** (`♬`)
  Plus a `◄` collapse toggle at the strip's bottom.
- Surfaces are navigation-only. No buffers, no derived views, no widgets
  ever mount here.
- **Invariant:** anything that needs to render a page's content or a
  query's results lives in the main pane tree, not the sidebar.
- The sidebar's active-surface and collapsed state persist per workspace
  (not per tab).

### Main pane tree

- Binary tree of vertical and horizontal splits.
- Leaves are buffers (see "Three buffer kinds" below).
- Resize via 1px drag handles; min weight 0.05; focus moves with hjkl /
  arrows / `<C-w> hjkl`; pane move with `⌘⇧hjkl` (Aerospace-style).

### Bottom status line

- See "Status line" section below.

## Overlays

- **⌘K Command Station**: verb palette + ambient launcher. The Station is
  the entry point for verbs (`vsplit`, `tabnew`, `promote`, `daily`, …) and
  for opening ambient buffers (`calendar`, `in-progress`, `dashboard`,
  `ai`). It is *not* the home for ambient buffer content — ambient buffers
  themselves live in the pane tree, so they are concurrent with editing
  rather than modal.
- **⌘I Peek**: transient overlay hosting one derived renderer at a time.
  Chord-cycles via `Tab` (forward) / `Shift-Tab` (back). Esc dismisses.
  Enter jumps. Telescope-shaped — keyboard-first, focused, single content
  area.
- **⌘G Fullscreen graph**: graph-of-workspace, takes over the main area.
  Local-graph-of-page is *not* this overlay — it's a derived renderer.
- **⌘, Settings**: fullscreen overlay over the main area. Workspace-level,
  scoped per section (general, devices, sync, mosaic, data).

## Three buffer kinds (core invariant set)

The pane tree's leaf nodes hold a discriminated union over exactly three
buffer kinds. Each kind has a one-line invariant enforced at the type
system level.

```typescript
type Buffer =
  | { kind: "page";    pageId: PageId }
  | { kind: "derived"; rendererName: string;
      binding: { mode: "follow" } | { mode: "pinned"; reference: Reference } }
  | { kind: "ambient"; ambientName: string };

type Leaf  = { type: "leaf"; id: LeafId; buffer: Buffer };
type Split = { type: "split"; id: SplitId; dir: "v" | "h"; ratio: number;
                children: [Node, Node] };
type Node  = Leaf | Split;
```

The pane tree algebra (split, merge, focus traversal, resize, persist) is
identical regardless of kind — the discriminated union lives at the leaf's
`buffer` field, not at the leaf-type level. Tripling the leaf types would
triple the algebra surface for zero gain.

### Page buffer

> **Invariant:** renders exactly one filesystem-backed page.

- Identified by `pageId` (stable path-based id).
- Renders via a **page-type renderer** keyed by the page's `type`
  frontmatter:
  - `note` → block outliner
  - `daily` → journal feed (today on top, older days scroll down)
  - `query` → results list (renders the page's `query::` DSL)
  - `scratch` → block outliner (same as `note`, different filter behavior)
  - `project` / `person` → typed renderers (deferred — out of MVP)
  - unknown `type:` → fallback to outliner with a small warning chip
- Adding a new page type means registering a new renderer. **Adding a new
  page type never requires a new buffer kind.**

### Derived buffer

> **Invariant:** pure function of a reference. Read-only. Cannot exist
> without a reference.

- Reference is a tagged union:

  ```typescript
  type Reference =
    | { kind: "page";  path: string }
    | { kind: "tag";   value: string }
    | { kind: "query"; dsl: string };
  ```

- **Renderers are registered with a declared input type.** The system
  refuses to mount a renderer with a non-matching reference type
  (e.g., cannot mount `backlinks-of-page` with a `tag` reference).
  Enforced at the type system (renderer's `accepts: K extends
  Reference["kind"]` parameter) and at runtime by the registry's mount
  guard (throws `RendererReferenceMismatch`).
- **Two binding modes:**
  - **Follow**: bound to "most-recently-focused page-buffer in this tab."
    Empty state when no page-buffer has ever been focused in this tab.
  - **Pinned**: bound to a fixed reference. Reference is to page identity,
    not to the open buffer; survives the closing of any page buffer that
    happened to show the same page.
- The renderer **must not** know whether it's in Follow or Pinned mode.
  The host resolves Follow into a concrete reference and passes it.
  Without this, every renderer reimplements the follow rule and they
  drift.
- **Follow resolution is read-time, not write-time.** No denormalized
  resolved-reference field on the buffer. Per-tab `lastFocusedPagePerTab`
  state; each follow-derived buffer reads it through `$derived`.
- Multiple derived buffers per tab allowed, including same-renderer +
  same-reference duplicates. Data (TanStack Query) is shared by reference
  key; UI state (scroll, expanded items, selection, current cascade
  mode) is per-component.
- Derived renderers are **host-agnostic**: the same renderer mounts inside
  a derived-buffer pane *and* inside Peek with no host knowledge. No
  `host` prop. Peek wraps in a popover frame; the pane wraps in a pane
  frame; the renderer renders content for `(reference, size)`.
- Initial renderer set:
  - `backlinks-of-page` (reference: page)
  - `outline-of-page` (reference: page)
  - `properties-of-page` (reference: page)
  - `local-graph-of-page` (reference: page; 1-hop)
  - `tasks-linked-to-page` (reference: page)

### Ambient buffer

> **Invariant:** workspace singleton not tied to a page. State is
> workspace-level. The same ambient buffer may render in multiple tabs
> simultaneously and share backing state across them, like a Vim buffer
> open in two windows.

- There is **no "close" of an ambient buffer at the state level.** There's
  only `unmountAmbient(tabId, leafId)` — pure UI lifecycle. Ambient state
  lives workspace-wide and is always available; mounting is per-tab. If
  you ever want to "reset the calendar," that's a separate verb
  (`resetAmbient(name)`), not "close."
- Initial set:
  - `calendar`
  - `today-in-progress`
  - `workspace dashboard`
  - `ai-workspace` (ships as **placeholder card** — "coming in a later
    phase" plus one disabled teaser action; full chat scope is a separate
    plan)
- The Command Station is the **launcher** — verbs open ambient buffers
  into a new pane (or focus an existing pane already showing them).
  Ambient buffers do not exist inside the Command Station UI.
- Per-project dashboards are NOT ambient. They are derived buffers with
  the project page as reference.
- Ambient state module convention: `web/src/lib/ambients/<name>/state.svelte.ts`
  exports reactive state + public API. Naming locks in Phase 5 to avoid
  drift across ambients.

## Renderer protocol (host-agnostic)

Renderers own their data fetching (TanStack Query keyed on the reference);
loading and error UI live inside the renderer, not negotiated with the
host. The host provides a resolved reference, size in cell-units, and an
intent sink:

```typescript
type Size = { cols: number; rows: number };

type NavigationIntent =
  | { kind: "open-page";  path: string;
      how: "replace" | "split-right" | "split-down" | "new-tab" }
  | { kind: "open-tag";   value: string }
  | { kind: "open-query"; dsl: string };

interface DerivedRendererProps<R extends Reference> {
  reference: R;          // already resolved; renderer is mode-blind
  size: Size;            // current host size; renderer reads, host re-passes on resize
  onNavigate: (i: NavigationIntent) => void;
}
```

The renderer doesn't mutate global navigation directly. It emits
intents; the host decides whether to split, replace, or open a tab.
Without this, Peek and pane hosts can't share renderers because they have
different navigation semantics (Peek closes-then-navigates; pane just
navigates).

## Renderer registries

**Three separate registries, not one.** Each kind has a different
protocol (page-type renderer takes a Page, derived takes a Reference +
size, ambient takes nothing because it reads workspace state). A unified
`(kind, name)` registry forces a union at the value level that obscures
the type-system guarantees you want.

Pattern (per registry):
- One module file per renderer, `export default` only (HMR-friendly under
  Vite).
- `web/src/lib/renderers/{page,derived,ambient}/index.ts` does explicit
  imports + `register(name, mod)` calls. No filesystem-discovery magic;
  greppable and debuggable.

```typescript
interface DerivedRenderer<K extends Reference["kind"]> {
  accepts: K;                                  // type-discriminator
  cascade: RendererCascade<DerivedRendererProps<Extract<Reference, { kind: K }>>>;
}

function mount(name: string, ref: Reference) {
  const r = derivedRegistry.get(name);
  if (r.accepts !== ref.kind)
    throw new RendererReferenceMismatch(name, r.accepts, ref.kind);
  // TS narrows r to its specific reference variant from here
}
```

## Renderer minimum sizing — cascade pattern

Each renderer module declares a **cascade** of modes, descending by
required size. The host picks the highest-min mode that fits; the
renderer doesn't conditionally branch on its own size.

```typescript
interface RendererCascade<P> {
  default: Component<P>;
  modes: ReadonlyArray<{
    minSize: Size;
    component: Component<P>;
    label?: string;                 // for status-line debug
  }>;
}
```

- Renderer module declares the cascade. Host picks. Workspace config can
  shift thresholds (accessibility) by transforming the cascade before the
  host consults it.
- Crossing a threshold swaps cascade members; that's one instantiation,
  not a re-render storm.
- Each cascade member receives `size` so it can lay out gracefully
  within its mode but doesn't choose modes.

Degraded modes for built-in renderers:
- `query` (wide table → compact list)
- `daily` (multi-day → today-only)
- `local-graph-of-page` (full graph → node-count chip)
- `outliner` — single mode, adapts within itself

## Focus rules

- "Most-recently-focused page-buffer" is **tab-scoped**.
- Focusing a derived or ambient buffer is allowed (for scroll, selection,
  click). **Focusing a derived or ambient buffer must not update
  last-focused-page state.**
- **Enforcement point: the `focusPane` mutation.** Single chokepoint;
  clicks and keyboard bindings and programmatic focus all route through
  it. The conditional that decides whether to update `lastFocusedPagePerTab`
  is data-driven (`leaf.buffer.kind === "page"`), not behavior-driven.

```typescript
function focusPane(tabId: TabId, leafId: LeafId) {
  const leaf = getLeaf(tabId, leafId);
  lastFocusedLeafPerTab.set(tabId, leafId);            // visual focus
  if (leaf.buffer.kind === "page") {
    lastFocusedPagePerTab.set(tabId, leaf.buffer.pageId); // follow source
  }
  // derived and ambient: intentionally do not update follow source
}
```

- `lastFocusedPagePerTab` is non-exported (or behind a private setter) so
  nothing else can mutate it. The rule comment lives at this single
  conditional.
- `⌘I` Peek with no page-buffer focused falls back to last-focused-page
  in the current tab. If none has ever been focused in this tab, Peek
  shows a quiet hint and is a no-op.
- **Tab-switch focus**: `lastFocusedLeafPerTab` persists alongside the
  pane tree in localStorage. Switching tabs restores the last focused leaf
  of the target tab. (Without this, every tab switch resets focus to root
  leaf on reload.)

## Per-renderer error boundary

Every page-buffer and derived-buffer leaf wraps its renderer in a Svelte
5 `<svelte:boundary>`. A crashing renderer fails soft to a "renderer
crashed: `<name>`, click to reload" card. Without this, one buggy renderer
breaks the cutover.

## Peek

- One renderer at a time. `Tab` cycles forward, `Shift-Tab` back, `Esc`
  dismisses, `Enter` jumps.
- Default cycle order: backlinks → outline → properties → tasks →
  local-graph.
- **Per-page-type first-shown memory**: a small workspace map of
  `pageType → preferredFirstRenderer` overrides the default (e.g., "for
  daily pages I always want outline first").
- Hide-list configurable per workspace so users can drop renderers they
  never want in Peek.

## Page-type renderers

Each page-type renderer follows the page-renderer protocol (sibling of the
derived-renderer protocol, taking a Page instead of a Reference) and
declares a cascade. Existing v4 components survive into v5 as renderers
via thin adapters (not rewrites):
- `BlockOutliner` → `note` + `scratch` page-types
- `JournalView` → `daily` page-type
- `QueryWidgetView` → `query` page-type

The adapter resolves the page from `pageId`, supplies the `(size,
onNavigate)` props, and otherwise leaves the inner component alone.

## Scratch (the draft-buffer answer)

Scratch is a **page type**, not a buffer kind. Daily remains first-class
and is not used as a scratch surface.

- `type: scratch` pages. Auto-named by timestamp:
  `scratch/2026-05-15-1423.md`.
- Opened via the palette verb `:scratch` **or** the leader chord
  `Space n s` (which-key tree: `Space n` = "new…", `n s` = scratch,
  alongside `n n` = note, `n d` = daily).
- Renders as outliner inside a page buffer.
- **Filtered out of default surfaces**: notes tree, search, recent,
  default queries. Visible only when explicitly included (`type:scratch`
  in a query, or a "show scratches" toggle in the tree surface).
- **Promote verb** (`:promote`): removes the `type: scratch` frontmatter,
  prompts for title and target location (default seed: `notes/` root),
  moves the file. Friction goes from zero (scratching) to small
  (promoting).
- **Optional prune sweep**: deletes scratch pages with no edits in N
  days. User-configurable, **default OFF**.

## Shared workspace state — hybrid TanStack + disk + in-memory query

Pinned set, recent queue, and search are not the same thing; they don't
share a storage shape.

- **Pinned**: user-curated list. Written to workspace state on disk so it
  syncs across devices. TanStack Query reads it from a small file;
  pinning mutates the file and invalidates the query.
  Key: `["pinned"]`.
- **Recent**: capped LRU side-effect of opening pages. Disk-persisted so
  it survives sessions. Same TanStack pattern.
  Key: `["recent"]`.
- **Search**: TanStack Query against the search backend (in-memory
  FlexSearch / Orama / whatever the eventual implementation is), keyed
  by query string. Not disk-persisted.
  Key: `["search", q]`.

Svelte 5 ergonomics: tiny consumer wrappers `usePinned()` /
`useRecent()` / `useSearch(q)` that return reactive `$derived` values.
The cache stays in TanStack Query; the wrappers are the only thin
Svelte-side layer.

**Propagation**: pinning a page from the sidebar writes to disk + calls
`queryClient.invalidateQueries({ queryKey: ["pinned"] })`. Both sidebar
and Station re-read on next tick — single source, no drift.

## Status line

Vim/Zellij-shaped. Bottom edge, ~24px tall, monospace.

- **Mode** (NORMAL/INSERT/VISUAL, when in a cm-editor; blank for
  non-editor buffers).
- **Focused buffer kind and name** (`page · 2026-05-15`, `derived ·
  backlinks of Project Alpha`, `ambient · calendar`).
- **Modified marker** (`●`) when the focused page buffer has unsaved
  changes.
- **Position** (line/block) when applicable.
- **Workspace name**.
- **Binding indicator**: `$derived` over the pane tree of the current tab
  + focus state. No separate bindings registry — the bindings *are* the
  pane tree contents.

```typescript
type BindingIndicator =
  | { kind: "page-has-followers"; count: number }
  | { kind: "derived-following"; resolvedPagePath: string | null }
  | { kind: "derived-pinned"; reference: Reference }
  | { kind: "ambient"; ambientName: string }
  | null;
```

For small pane counts per tab (10s, not 1000s), the tree walk is fine.
Memoize by tab-tree-version if the tree ever grows.

## Tabs and pane trees

- Each tab owns its own pane tree.
- Tabs are workspace-scoped. Closing a tab does not affect any other tab.
- Ambient buffer **state** is workspace-level; ambient buffer **mounts**
  are per-tab. Unmounting an ambient pane in tab A leaves state intact;
  tab B is unaffected because it reads workspace state, not A's mount.
- Per-tab last-focused-leaf persists with the pane tree for tab-switch
  restoration.

## Persistence — `tesela:prism:state`

- One top-level localStorage key: `tesela:prism:state` (the `prism4`
  prefix from v4 is a wart in the v5 era).
- Versioned envelope inside (`_v: 3` for v5; v4's `tesela:prism4:v1` is
  `_v: 2`).
- One-shot rename + migration on first v5 boot:

```typescript
const KEY = "tesela:prism:state";
function load(): StateV3 {
  const raw = localStorage.getItem(KEY) ?? localStorage.getItem("tesela:prism4:v1");
  if (!raw) return defaultState();
  const p = JSON.parse(raw);
  switch (p._v) {
    case 1: return v1to3(p);
    case 2: return v2to3(p);
    case 3: return p;
    default: return defaultState(); // unknown future
  }
}
```

- v4 → v5 migration walks the v4 tree and maps leaves:
  - `editor` → `page` buffer
  - `context` (backlinks / outline / properties modes) → `derived` with
    the matching renderer name and follow binding
  - `widget` → `ambient` via a small name table
  - `graph` → drop with a migration-log entry
  - `dashboard` → `ambient · workspace-dashboard` by default (heuristic;
    will be wrong sometimes, acceptable)
- Migration is **idempotent** and has **golden-file tests** against
  captured v4 blobs.
- **One-time changelog modal** on first v5 boot lists dropped or
  migrated panes ("Prism v5: 2 graph panes dropped, 1 dashboard
  converted to workspace dashboard, all editing preserved") so users
  don't think they lost work.

## Opening flow

On fresh boot with no saved state, the workspace seeds:
- 1 tab
- 1 page buffer with today's **daily** loaded (auto-created if missing)
- Sidebar **open** with the **notes tree** surface active
- Cursor in the daily, ready to type

On boot with saved state, restore the persisted pane tree, tabs,
last-focused leaf per tab, and sidebar state.

## Rejected alternatives

- **Right sidebar for context** (parallel to left nav). Two always-on
  rails consume too much horizontal real estate; user requested a singular
  Neovim-style sidebar.
- **Bottom panel for context** (IDE-style). Vertical space matters for
  prose; bottom panels are culturally second-class; less flexible than
  splits.
- **Peek-only for context** (no paneable context). Some workflows want
  context sticky alongside the editor, not transiently popped open per
  consult.
- **Free-form pane kinds**. The original v4 grab-bag; rejected because
  the five kinds had no shared invariant.
- **Two-kind model** (page + context only). Rejected because the
  context-kind invariant decays into a sub-mode grab-bag (backlinks /
  outline / properties / AI / tasks / diff / …); deriving by reference is
  the clean version.
- **Scratch as a separate buffer kind**. Rejected; keep "everything is a
  page" by making scratch a `type:`.
- **Scratch as Daily**. Rejected; Daily remains first-class and
  outliner-shaped, not a dumping ground.
- **Local graph as a pane kind**. Rejected; it's a derived renderer with
  a page reference, like any other.
- **Build-alongside `/v5` route**. Rejected per product decision; cutover
  is rip-and-replace, main is the rollback.
- **Feature flag for v4/v5**. Rejected; branching at every fork creates
  more drift than rip-and-replace.
- **Unified `(kind, name)` renderer registry**. Rejected; the three kinds
  have different protocols and the union obscures type-system guarantees.
- **Filesystem-discovery for renderer modules**. Rejected; breaks under
  HMR, hides ordering bugs, hard to debug when a renderer doesn't show
  up. Explicit imports + `register()` calls.
- **`host` prop on renderers**. Rejected; reintroduces host knowledge
  inside the renderer, exactly the contract we're trying to enforce. The
  renderer renders for `(reference, size)`; the host wraps in chrome.
- **Renderer self-decides degraded mode**. Rejected; either causes
  re-render storms on resize or every renderer invents its own breakpoint
  scheme. Cascade owned by renderer module; host picks.
- **Separate bindings registry for status line**. Rejected; the bindings
  *are* the pane tree contents; a registry would be a denormalized mirror
  that drifts.
- **New top-level localStorage key for v5**. Rejected; orphans v4 data on
  user machines. One-shot rename to `tesela:prism:state` with in-place
  migration.
