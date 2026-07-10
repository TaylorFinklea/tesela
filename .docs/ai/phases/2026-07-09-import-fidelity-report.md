# Import-fidelity audit: Taylor's real Logseq graph vs the Tesela importer

**Bead**: tesela-nnm.1 · **Date**: 2026-07-09 · **Method**: sandbox import of a
read-only copy of `~/logseq` into a fresh, network-isolated mosaic; structural
verification via JQL over the live server API; byte-level diffing of the
mosaic's `notes/*.md` across each boot phase.

## Headline verdict

**Task/schedule fidelity survives import exactly** (see Count comparison).
But the mandatory one-time bootstrap step after any fresh import/mosaic
(`TESELA_LORO_RESEED=1`) **silently destroys non-bulleted content** —
confirmed on Taylor's real graph: every top-level Markdown heading (19/19)
deleted graph-wide, and one page (his 7-query Logseq dashboard, `TODO.md`)
reduced to a single empty bullet. This is filed as **tesela-myh (P1)** and
should block the cutover trial gate (tesela-nnm.3) until resolved or
mitigated. Everything else found was either already tracked by the
tesela-8zd/tesela-ewj epics (evidence appended, not duplicated) or a small,
well-scoped new gap (4 new beads below).

## Method

1. `cp -R ~/logseq` → scratchpad `logseq-copy/` (source never touched again).
2. Fresh sandbox mosaic (`tesela init`), `TESELA_GROUP_KEY_FILE_STORE=1`, no
   `TESELA_RELAY_*` — network-isolated throughout.
3. `tesela import-logseq --source logseq-copy --dry-run` then applied.
4. Snapshot `notes/*.md` immediately post-import (pre-boot).
5. Boot `tesela-server` **without** `TESELA_LORO_RESEED` first — confirms the
   existing A9b safety gate (`stamp_existing_notes`) protects the file-only
   pass (verified: zero bytes changed on any pre-existing file).
6. Snapshot again, reboot the **same mosaic** **with**
   `TESELA_LORO_RESEED=1` — this is the documented one-time bootstrap step —
   and diff against the pre-reseed snapshot to isolate exactly what changes.
7. JQL queries via `POST /search/query` (`{"dsl": "...", "group": "..."}`)
   against the reseeded, fully-booted server, cross-checked against
   fence-aware Python re-derivation of the source grep counts (excluding
   matches inside ` ``` ` code fences, which the importer correctly never
   converts).

## Count comparison (source-of-truth vs mosaic, JQL-verified)

All source counts below are **fence-aware** (re-derived in Python, excluding
matches inside triple-backtick code blocks — the importer correctly skips
those, so naive `grep` overcounts by exactly the in-fence occurrences).

| Feature | Source (fence-aware) | Mosaic (JQL) | Verdict |
|---|---|---|---|
| DONE → `status:: done` | 757 | 757 | ✓ exact |
| TODO → `status:: todo` | 122 | 122 | ✓ exact |
| DOING → `status:: doing` | 7 | 7 | ✓ exact |
| LATER → `status:: backlog` | 9 | 9 | ✓ exact |
| CANCELED → `status:: canceled` | 13 | 13 | ✓ exact |
| **Total tasks** | **908** | **908** | ✓ exact |
| `[#A]` → `priority:: high` | 109 | 108 | −1 (gap: tesela-ml6) |
| `[#B]` → `priority:: medium` | 59 | 59 | ✓ exact |
| `[#C]` → `priority:: low` | 43 | 42 | −1 (gap: tesela-ml6) |
| SCHEDULED → `scheduled::` | 168 | 168 | ✓ exact |
| DEADLINE → `deadline::` | 73 | 73 | ✓ exact |
| Repeaters (`.+1w` etc., 13 occ) → `recurring::` | 13 | 0 | ✗ total loss (gap: tesela-car) |
| Journals (429 files, 76/10/3/91/135/114 by year 2021-2026) | 429 | 429 notes, filenames `YYYY-MM-DD.md` | ✓ exact |
| Pages (49, minus `contents.md` filtered by design) | 49 | 48 imported | ✓ (by design) |
| Namespace pages (`Foo___Bar.md`) | 21 | 21 flattened, parent → tag | ✓ exact |
| Assets | 268 files / 90MB | 268 copied / 90MB | ✓ exact |
| Asset refs `../assets/` → `../attachments/` | 94 occ / 90 unique | 90/90 resolve to real files | ✓ 100% |
| `collapsed::` (outliner fold state) | 175 occ | 0 (stripped) | Dropped by design — no Tesela concept exists (gap: tesela-6yo, Lead triage) |
| `id::` (block anchors) | 16 occ | 0 (stripped) | Dropped — breaks block refs (tracked: tesela-8zd.7, evidence appended) |
| Block refs `((uuid))` | 4 occ, all 4 point at in-graph `id::` anchors | 4 preserved as literal text, 0/4 resolve | Dangling (tracked: tesela-8zd.7) |
| `:LOGBOOK:` drawers | 7 files | 7/7 pass through as literal text | Tracked: tesela-8zd.12 (evidence appended) |
| `#+BEGIN_QUERY` blocks | 8 occ (2 files: `TODO.md`×7, one journal×1) | 8/8 converted to fenced ` ```query ` blocks **at import time** | Converts correctly, then **destroyed by reseed** for 7/8 (see below) |
| Wikilinks `[[...]]` | 620 occ / 154 unique targets (16 namespaced, 1 multi-word) | Simple targets resolve; namespaced/multi-word (17/154) confirmed NOT to backlink via live API test | Tracked: tesela-8zd.5 (evidence appended) |
| Whiteboards | 7 files | 7/7 hard-skipped with clear reason | Handled — documented no-op, correct |
| Org-style `<date>` stamps | 249 occ (= 175 SCHEDULED + 74 DEADLINE; **zero** bare inline prose timestamps found) | All captured by `LOGSEQ_DATE_RE` (date + optional HH:MM); repeater suffix dropped | See recurring:: row above |

## The critical finding: non-bullet content deleted by the reseed bootstrap

`note_tree.rs`'s `parse_note → serialize_note` round trip has no
representation for content that isn't a bullet line — a fact its own module
docs already state ("non-bullet headings/prose (deleted)..."). An audit on
2026-06-09 (A9b) added a byte-for-byte safety gate
(`stamp_is_content_preserving`) that protects the **file-only**
`stamp_existing_notes` boot pass: if rewriting a note would change anything
beyond bid comments, the file is left untouched and a warning is logged
instead. This gate works — confirmed here: booting the sandbox **without**
reseed left every one of 512 notes byte-identical (473/486 imported notes
were flagged "non-canonical" and simply skipped, 0 bytes changed).

**`TESELA_LORO_RESEED=1` — the documented one-time bootstrap step for a
canonical device after any fresh import — pushes the exact same content
through the exact same lossy `parse_note`/`materialize_note` transform, with
NO equivalent gate.** Rebooting the identical sandbox mosaic with reseed
enabled rewrote 509/512 notes; diffing pre- vs post-reseed content (bid
comments stripped, blank-line-after-frontmatter normalized — the one
genuinely cosmetic difference, see tesela-mxk) showed **103/512 notes (20%)
have real differences beyond that safe normalization.**

Confirmed severities:
- **All 19 of 19** top-level Markdown `# heading` lines graph-wide are
  deleted outright (19 before reseed, 0 after — verified file-by-file).
- **Catastrophic**: `TODO.md` — Taylor's Logseq dashboard with 7 saved
  `#+BEGIN_QUERY` blocks, which converted correctly to fenced ` ```query `
  blocks at import time — is reduced from 2977 bytes to 156 bytes by reseed:
  every query is gone, replaced by a single empty bullet.
- A code-fenced ASCII diagram (`ai-business.md`) and multi-paragraph prose
  (`operating-system-nixos.md`: install notes, several lines) were also
  confirmed deleted.
- The remaining ~84 of the 103 are lower-severity reformatting (heavily
  indented lines snapped to 2-space indent; whitespace-only blank
  continuation lines collapsed) — not necessarily data loss, but worth a
  closer pass once the headline cases are fixed.

Filed as **tesela-myh** (P1, `tier_floor: lead`, `complexity: XL`), wired as
`blocks: tesela-nnm.3` (the cutover runbook gate) since this sits directly on
the standard bootstrap path. Evidence also appended to **tesela-ewj.1**
(importer-through-engine) — that bead's own acceptance criteria already
anticipated "silent mangling," but its fix (single writer, no double-write)
does not by itself resolve the root cause, since the same lossy transform
will still fire on first hydration of a non-bulleted page.

A secondary, much smaller bug compounds this one's blast radius: the
importer's rendered templates omit the blank line Tesela's canonical format
expects after frontmatter, which alone accounts for most of the 473/486
"non-canonical" flags at boot (confirmed lossless — bid comments + one
blank line, nothing else — for files with no other structural issue). Fixing
that (**tesela-mxk**, small) would shrink the flagged set from 473 down to
roughly the ~32 files that have genuine non-bullet content, isolating a
clean signal for tesela-myh's regression test.

## Per-feature verdict table

| Feature | Verdict | Evidence |
|---|---|---|
| Task status (TODO/DOING/DONE/LATER/CANCELED) | **Handled** — exact | 908/908 JQL match |
| Priority `[#A/B/C]` on task bullets | **Handled** — exact | 219/220 match (see gap for the 1 non-task-bullet class) |
| Priority-only bullets (no task marker) | **Degraded** | 2 occurrences silently untouched — tesela-ml6 |
| SCHEDULED/DEADLINE dates | **Handled** — exact | 241/241 JQL match |
| Repeaters → `recurring::` | **Dropped** | 13/13 lost, no recovery path — tesela-car |
| Journals (filename, per-year counts) | **Handled** — exact | 429/429 |
| Pages + namespace flattening | **Handled** — exact | 48/48 imported, 21/21 namespaced correctly tagged |
| Assets (copy + URL rewrite) | **Handled** — exact | 268/268 copied, 90/90 refs resolve |
| `collapsed::` (outliner fold state) | **Dropped by design** | No Tesela concept exists — tesela-6yo (Lead triage) |
| `id::` block anchors | **Dropped** | Breaks block refs — tesela-8zd.7 (evidence appended) |
| Block refs `((uuid))` | **Degraded → dangling** | Text preserved, 0/4 resolve — tesela-8zd.7 |
| `:LOGBOOK:` drawers | **Degraded → literal passthrough** | 7/7 files, unreadable clutter — tesela-8zd.12 |
| `#+BEGIN_QUERY` → fenced query blocks | **Handled at import, then destroyed by reseed** | 8/8 correct pre-reseed, 1/8 survives post-reseed — tesela-myh |
| Wikilinks (simple) | **Handled** | Resolves + backlinks even without a target file |
| Wikilinks (namespaced/multi-word) | **Degraded** | 17/154 unique targets don't backlink — tesela-8zd.5 |
| Whiteboards | **Handled — correct no-op** | 7/7 hard-skipped with a clear reason |
| Non-bullet content survival through reseed | **Critical gap** | 103/512 notes altered, 19/19 headings + 1 full page destroyed — tesela-myh |
| Stamp-gate false-positive rate | **Degraded** | 473/486 (97%) flagged due to one missing newline — tesela-mxk |

## Gaps filed (new beads, all `discovered-from:tesela-nnm.1`)

| Bead | Priority | Tier | Summary |
|---|---|---|---|
| **tesela-myh** | P1 | lead / XL | Non-bullet content deleted by reseed/materialize — the critical finding above. Blocks tesela-nnm.3. |
| **tesela-car** | P2 | senior / S | SCHEDULED/DEADLINE repeaters dropped, `recurring::` never set, no recovery path once imported. |
| **tesela-ml6** | P3 | junior / S | Priority-only bullets (no task marker) don't convert to `priority::`. |
| **tesela-mxk** | P3 | junior / S | Importer omits blank line after frontmatter → 97% false-positive stamp-gate flags. |
| **tesela-6yo** | P3 | lead / M | `collapsed::` outliner state has no Tesela equivalent — needs a scope decision, not a mechanical fix. |

## Evidence appended to existing tracked beads (not duplicated)

- **tesela-8zd.12** (LOGBOOK import) — confirmed 7/7 files, both placement
  patterns (block-property continuation vs own bullet) observed and
  documented.
- **tesela-8zd.5** (wikilink normalization) — confirmed via live
  `GET /notes/{id}/backlinks`: namespaced links return zero backlinks while
  simple links resolve even without a target file existing. 17/154 unique
  targets in Taylor's graph affected (16 namespaced + 1 multi-word).
- **tesela-8zd.7** (block refs bid-native) — confirmed all 4 of Taylor's
  block refs point at genuine in-graph `id::` anchors (not external), and
  100% of those anchors are stripped on import, so all 4 are provably
  dangling today.
- **tesela-ewj.1** (importer writes through the engine) — pointed at
  tesela-myh as the concrete evidence for that bead's own "silent mangling"
  acceptance-criteria note, with a sequencing recommendation.

## Runbook implication

**Do not run `TESELA_LORO_RESEED=1` against a freshly imported real graph
until tesela-myh lands or a stopgap is in place.** Today, the standard
one-time bootstrap step is confirmed to destroy content — including a
complete Logseq dashboard page — on Taylor's actual graph. The cutover
runbook (tesela-nnm.3) should not clear its gate until this is resolved,
which is why tesela-myh is wired as a hard `blocks` dependency.

## Sandbox cleanup

The read-only `~/logseq` source was never modified (verified: only the
scratchpad copy was touched throughout). The scratchpad copy, snapshots, and
sandbox mosaics were removed at the end of this session; no residue was left
outside `.docs/ai/phases/` in the repo.
