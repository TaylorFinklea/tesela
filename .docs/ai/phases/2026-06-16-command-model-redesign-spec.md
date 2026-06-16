# Command-model redesign — design (APPROVED 2026-06-16)

Status: **APPROVED** by Taylor ("good to go", 2026-06-16) after reviewing the v2 interactive
mockup. Ready for an implementation plan. **No production code until the PLAN is reviewed.**

## Decisions locked (2026-06-16)
- **Insert-mode leader chord = `Ctrl+,`** (rebindable via the L3 override layer).
- **Leader buckets = `g` go-to · `w` windows · `b` buffers · `n` new · `i` insert · `p` properties · `v` views · `a` actions · `t` toggle · `,` config**, + leaves `SPC SPC` palette / `SPC /` station.
- **`:` colon narrows to exact-verbs-only** (no note interleave; that's ⌘K). Fold the hand-duplicated colon builtins (peek/graph) back into the registry.
- **Slash = type-to-filter (Logseq) + `Ctrl+letter` accelerators** (see §1).
- **Leader layout = wide multi-column which-key grid** (see §2).

Grounded in the command-surface map (workflow `wf_37efdece-251`): all four surfaces
are filters over ONE `commandRegistry` (`web/src/lib/command-registry.svelte.ts`);
the redesign is mostly a per-surface *ownership* field + a leader-bucket model +
slash paring — not a rewrite.

## Charter (confirmed with Taylor 2026-06-16)

| Surface | Job | Owns |
|---|---|---|
| **`/` slash** | inline **content insertion + context-aware properties** while typing | the 8 insertion verbs; a context-aware Properties entry |
| **Leader (SPC)** + **insert-mode chord** | the **complete keyboard command map**, which-key buckets | EVERY command, in a bucket |
| **⌘K** | **universal fuzzy search** | commands + notes/pages |
| **`:` colon** | **power-user verb typing** (exact verbs, no note interleave) | verbs |

Overlap is allowed but each command has a *primary home*. `:` and ⌘K stop being twins:
`:` = exact verbs only; ⌘K = fuzzy over verbs + notes. Fold the hand-duplicated colon
builtins (peek/graph) back into the registry.

## 1. Slash (`/`) — type-to-filter (Logseq) + Ctrl-accelerators (REVISED 2026-06-16 per Taylor)

Today `getSlashTree` (BlockEditor.svelte:1407-1490) is a CHORD menu (single bare letters
= accelerators) AND it hoists the focused block's tag PropertyDefinitions to the top level
(~10+ rows on a Task block). Redesign — make it a **type-to-filter** menu like Logseq DB:

- **Default interaction = TYPE the command name to filter.** Typing `/` opens the menu;
  you keep typing (`/prop` → Properties, `/head` → Heading); the list narrows by
  fuzzy/prefix match on the label; **Enter** selects the highlighted match; **↑/↓** navigate.
  "Sometimes just type the thing and hit enter — faster than navigating." (Taylor)
- **Ctrl+letter accelerators (express lane), active ONLY while the menu is open.**
  `Ctrl+P` → Properties, `Ctrl+H` → Heading, etc. — direct jump for the regulars without
  typing the whole name. The bare-letter chords of today are REPLACED by this: bare keys
  type-to-filter, the modifier is the accelerator (so they don't fight).
- **Contents (unchanged from the pared design):** the 8 insertion verbs (Heading, Task,
  Link, Tag, Date, Template, Query, Collection) + the **context-aware `Properties`** entry
  (Task → Status/Priority/Deadline/Scheduled/Points; other types → theirs; untyped → All
  properties). No more hoisted top-level props. `New widget` → leader `new` bucket.
- This makes slash a *scoped* type-to-filter picker (insertion + context-props), interaction-
  consistent with ⌘K (fuzzy) but narrower in scope; the leader stays chord-press (below).

## 2. Leader (SPC) — which-key buckets, every command

The leader already auto-builds a which-key tree from each command's `chord: string[]`
(leader-tree.svelte.ts) — but only ~18 commands carry a chord; ~30 are palette/`:`-only.
Redesign = give EVERY command a chord (a bucket home) + name the buckets deliberately.
Proposed v1 taxonomy (refine in mockup):

- **g · go to** — daily, date…, yesterday, tomorrow, graph, calendar, inbox, agenda, dashboard, AI
- **w · windows** — split vert, split horiz, close pane, focus ←/↓/↑/→
- **b · buffers/tabs** — new tab, close tab, jump to tile
- **n · new** — note…, scratch, widget
- **i · insert** (in-block; mirrors slash for the keyboard path) — heading, task, link, tag, date, template, query, collection
- **p · properties** (set on block) — status, priority, deadline, scheduled, points, all…
- **v · views** (derived of current note) — backlinks, outline, properties, tasks, local graph, tag instances, tag backlinks
- **a · actions** (on block/note) — promote, → tag, → note, rename slug, prune scratches, delete tag, skip occurrence
- **t · toggle** — peek, (vim/theme later)
- **, · config** — general, devices, sync, mosaic, data, keymap
- leaves: **SPC SPC** command palette (⌘K) · **SPC /** command station

Every registry command lands in exactly one bucket → the leader becomes the complete,
discoverable keyboard map (emacs-2.0). Driven by `category` (already on every command) +
a small bucket-label table, OR a new `bucket` field — TBD in implementation.

**Layout (REVISED 2026-06-16 per Taylor): a WIDE multi-column which-key grid**, like emacs
`which-key` / neovim `which-key.nvim` — keys laid out in 2–3 columns so the popup is wide
and short, NOT a tall single column. Navigation unchanged (press a bucket letter to descend,
Esc/Backspace ascends, breadcrumb shown). Chord-press stays the leader's model (vs slash's
type-to-filter) — matching the references (which-key = chords; M-x/palette = type-to-filter).

## 3. Insert-mode invocation (NEW — doesn't exist today)

**`Ctrl+,`** opens the SAME leader overlay without leaving insert mode (reuses
`openLeader()`; the `Ctrl+,` in ChordMenu.svelte:11 is a dead comment to make real).
Rebindable via the L3 override layer. The `i` filter key is reserved INSIDE the menu, not
the opener; confirm `Ctrl+,` doesn't collide with a cm-editor/vim insert binding during impl.

## 4. Intentional overlap (examples)

- `daily`: leader `g d` + ⌘K + `:` — NOT slash.
- `heading`: slash + leader `i h` + ⌘K — NOT `:` (gate the surface:"global" leak).
- properties: leader `p …` + slash `/p ▸` (context-aware) — NOT ⌘K/`:` top-level.
- derived views: leader `v …` + ⌘K + `:`.

## Registry changes this implies (for writing-plans, post-approval)

1. Per-surface ownership field (replace binary `surface: global|editor` with a per-surface
   set), threaded through `available(ctx)` (the one shared gate).
2. Bucket metadata (named buckets + ordering) — extend `CHORD_GROUP_LABELS` or add a field.
3. Give the ~30 chord-less commands a `chord` (bucket home).
4. Slash: drop the hoisted props + fallback; add the `/p` context-aware entry.
5. Insert-mode chord handler → `openLeader()`.
6. Narrow `:` (verbs only) + fold peek/graph builtins into the registry.

## Mockup brief (interactive)

Self-contained HTML, Tesela warm-dark theme (dark bg, coral `#FF6B5A`-ish accent,
monospace, chord-chip style from the screenshot). Interactive: (a) a journal block line
where typing `/` opens the **pared** slash menu (8 verbs + `Properties ▸` → context-aware
submenu for a #Task block); a before/after toggle showing today's 16-flat vs the new;
(b) pressing the insert-mode chord opens the **bucketed leader**; keyboard + click descend
into buckets (g/w/b/n/i/p/v/a/t/,) showing every command homed; Esc/Backspace ascends.
Goal: Taylor can feel the paring + the bucket navigation and sign off (or redirect).
