# Agenda / Today View (web) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an Agenda ambient buffer — a scrollable-forward calendar of all dated tasks and events in the mosaic, with recurring tasks projected forward and inline mark-done / reschedule / skip.

**Architecture:** A new `POST /agenda` server endpoint returns expanded occurrences within a `[from, to]` window — including projected future occurrences of recurring tasks (computed via the existing `tesela_core::recurrence` engine). The web client adds a new `agenda` ambient buffer registered the same way Calendar / Dashboard are; `Agenda.svelte` fetches from `/agenda`, renders day-bucketed rows (`AgendaDay` / `AgendaRow`), and routes mark-done / reschedule / skip through existing block-save paths (`updateNote`, the DatePicker, `skipRecurrence`).

**Tech Stack:** Rust (`tesela-core`, `tesela-server`), SvelteKit 2 / Svelte 5 runes, Vitest (`pnpm test:unit`).

**Reference spec:** `.docs/ai/phases/2026-05-22-agenda-view-design.md`

---

## File Structure

- `crates/tesela-core/src/query.rs` — **modify.** New `AgendaRow` struct.
- `crates/tesela-core/src/traits/search_index.rs` — **modify.** New `agenda_blocks(from, to, include_done)` trait method.
- `crates/tesela-core/src/db/sqlite.rs` — **modify.** SQLite impl of `agenda_blocks`: query dated blocks, expand recurring blocks via `recurrence::advance` within the window, sort.
- `crates/tesela-server/src/routes/agenda.rs` — **create.** `POST /agenda` handler.
- `crates/tesela-server/src/routes/mod.rs` — **modify.** Route registration.
- `web/src/lib/api-client.ts` — **modify.** New `getAgenda(from, to, includeDone)` method.
- `web/src/lib/types/AgendaRow.ts` — **create** (or regenerate from Rust via `ts-rs` if the repo uses that for shared types).
- `web/src/lib/ambients/agenda/index.ts` — **create.** `AmbientRenderer` definition.
- `web/src/lib/ambients/agenda/Agenda.svelte` — **create.** Top-level component (fetch, window state, infinite scroll, hide-done toggle).
- `web/src/lib/ambients/agenda/AgendaDay.svelte` — **create.** Day section.
- `web/src/lib/ambients/agenda/AgendaRow.svelte` — **create.** Single row.
- `web/src/lib/renderers/register.ts` — **modify.** Register the agenda ambient.
- `web/src/lib/v4/commands.ts` — **modify.** Add `:agenda` to the `AMBIENTS` array.

---

## Task 1: `AgendaRow` type + `agenda_blocks` trait method

**Files:**
- Modify: `crates/tesela-core/src/query.rs`
- Modify: `crates/tesela-core/src/traits/search_index.rs`

Define the row shape the endpoint returns, plus the trait method. No projection logic yet — that's Task 2.

- [ ] **Step 1: Write the failing test**

Add to `crates/tesela-core/src/query.rs` (or wherever `DayMarkers` is defined — read the file to locate; place `AgendaRow` next to it):

```rust
#[cfg(test)]
mod agenda_row_tests {
    use super::*;

    #[test]
    fn agenda_row_round_trips_via_serde() {
        let r = AgendaRow {
            block_id: "b1".to_string(),
            source_note_id: "2026-05-22".to_string(),
            occurrence_date: "2026-05-22".to_string(),
            occurrence_time: Some("14:00".to_string()),
            kind: AgendaRowKind::Task,
            overdue: false,
            recurrence: Some("weekly".to_string()),
            is_anchor: true,
            text: "do this thing".to_string(),
            status: Some("todo".to_string()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: AgendaRow = serde_json::from_str(&json).unwrap();
        assert_eq!(back.block_id, "b1");
        assert_eq!(back.kind, AgendaRowKind::Task);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-core --lib agenda_row` → FAIL (`AgendaRow` undefined).

- [ ] **Step 3: Add the types**

In `crates/tesela-core/src/query.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "lowercase")]
pub enum AgendaRowKind {
    Task,
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct AgendaRow {
    pub block_id: String,
    pub source_note_id: String,
    /// YYYY-MM-DD of the occurrence (the anchor date for non-recurring; a
    /// projected future date for recurring projections).
    pub occurrence_date: String,
    /// Optional HH:MM if the source date carries a time.
    pub occurrence_time: Option<String>,
    pub kind: AgendaRowKind,
    /// `true` if `occurrence_date < today_at_query_time`.
    pub overdue: bool,
    /// The block's `recurring::` value (canonical string) when projecting; `None` otherwise.
    pub recurrence: Option<String>,
    /// `true` for the block's current anchor; `false` for projected future occurrences.
    pub is_anchor: bool,
    /// The block's text (sans `status::`/`deadline::`/etc. property lines).
    pub text: String,
    /// `status::` value (`"todo"`, `"done"`, etc.) for task rows; `None` for events.
    pub status: Option<String>,
}
```

(Adapt the `ts(export_to = ...)` path to match how other types in `query.rs` already do it — verify against existing `DayMarkers` if it derives `TS`.)

- [ ] **Step 4: Add the trait method**

In `crates/tesela-core/src/traits/search_index.rs`, add to the `SearchIndex` trait:

```rust
    /// Return the agenda rows in [from, to] (inclusive both ends, `YYYY-MM-DD`).
    /// Expands recurring blocks via `recurrence::advance` so each projected
    /// future occurrence within the window is its own row. Done tasks are
    /// excluded unless `include_done` is true. Sorted by
    /// (occurrence_date, occurrence_time, block_id).
    async fn agenda_blocks(
        &self,
        from: &str,
        to: &str,
        include_done: bool,
    ) -> Result<Vec<crate::query::AgendaRow>>;
```

Add a stub method on any concrete impls that aren't the SQLite one (a `unimplemented!()` or `todo!()` is fine; only the SQLite impl is used by the server in this plan).

- [ ] **Step 5: Run tests**

Run: `cargo test -p tesela-core --lib agenda_row` → PASS.
Run: `cargo build -p tesela-core` → clean.

- [ ] **Step 6: Commit**

```bash
git add crates/tesela-core/src/query.rs crates/tesela-core/src/traits/search_index.rs
git commit -m "feat(core): AgendaRow type + agenda_blocks trait method"
```

---

## Task 2: SQLite `agenda_blocks` — query + recurrence projection

**Files:**
- Modify: `crates/tesela-core/src/db/sqlite.rs`

Implement `agenda_blocks` against the SQLite index. Fetch dated blocks within (or relevant to) the window, expand each recurring block via `recurrence::advance`, return a flat sorted list.

- [ ] **Step 1: Locate the right place**

Run: `rg -n "impl SearchIndex for|calendar_marks|execute_query" crates/tesela-core/src/db/sqlite.rs` to find the `SearchIndex` impl block + the most similar existing method to mirror (`calendar_marks` is the closest — it also walks blocks within a date window). Read its body to see how blocks-with-properties are fetched from the SQLite index.

- [ ] **Step 2: Write the failing test**

Add to the existing test module in `sqlite.rs` (find `#[cfg(test)] mod tests` and append):

```rust
#[tokio::test]
async fn agenda_blocks_returns_dated_blocks_in_window() {
    let idx = TestIndex::fresh().await; // mirror the test-harness helper used by calendar_marks tests
    idx.put_note("a", "- buy milk\n  scheduled:: 2026-05-22\n  tags:: Task\n  status:: todo\n").await;
    idx.put_note("b", "- party scheduled:: 2026-05-23 14:00\n").await;
    idx.put_note("c", "- already done\n  scheduled:: 2026-05-22\n  tags:: Task\n  status:: done\n").await;
    let rows = idx.agenda_blocks("2026-05-22", "2026-05-25", false).await.unwrap();
    assert_eq!(rows.len(), 2); // "c" excluded (done)
    assert_eq!(rows[0].block_id, /* the buy-milk block id */);
    assert_eq!(rows[0].kind, AgendaRowKind::Task);
    assert_eq!(rows[1].kind, AgendaRowKind::Event);
    assert_eq!(rows[1].occurrence_time, Some("14:00".to_string()));
}

#[tokio::test]
async fn agenda_blocks_projects_recurring_forward() {
    let idx = TestIndex::fresh().await;
    // Weekly task starting Fri May 22, projected over a 3-week window.
    idx.put_note("a",
        "- weekly review\n  scheduled:: 2026-05-22\n  recurring:: weekly\n  tags:: Task\n  status:: todo\n"
    ).await;
    let rows = idx.agenda_blocks("2026-05-22", "2026-06-12", false).await.unwrap();
    let dates: Vec<&str> = rows.iter().map(|r| r.occurrence_date.as_str()).collect();
    assert_eq!(dates, vec!["2026-05-22", "2026-05-29", "2026-06-05", "2026-06-12"]);
    assert!(rows[0].is_anchor);
    assert!(!rows[1].is_anchor);
    assert!(!rows[2].is_anchor);
    assert!(!rows[3].is_anchor);
}

#[tokio::test]
async fn agenda_blocks_respects_recurrence_count() {
    let idx = TestIndex::fresh().await;
    idx.put_note("a",
        "- thrice\n  scheduled:: 2026-05-22\n  recurring:: weekly count 3\n  recurrence_done:: 0\n  tags:: Task\n  status:: todo\n"
    ).await;
    let rows = idx.agenda_blocks("2026-05-22", "2026-12-31", false).await.unwrap();
    assert_eq!(rows.len(), 3); // count 3 caps the projection
}
```

Adapt the test-harness helper names to whatever `sqlite.rs`'s existing test module already uses (it has a fresh-index helper — find and mirror it).

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p tesela-core --lib agenda_blocks` → FAIL (method missing or returning wrong data).

- [ ] **Step 4: Implement `agenda_blocks`**

Add the method to the `impl SearchIndex for SqliteIndex` block. Algorithm:

1. **Today** = `chrono::Local::today()`; parse `from` and `to` as `NaiveDate`.
2. **Base query** — fetch all blocks (with their note_id, text, properties dict) where `has:deadline OR has:scheduled`. Reuse `execute_query` with a `ParsedQuery` of `kind:block (has:deadline OR has:scheduled)`, or query the properties table directly — whichever the existing `calendar_marks` impl uses for the same data. If `include_done` is `false`, also filter `-status:done` (the query DSL supports negation; or filter in Rust after the fetch — small mosaics make this cheap).
3. **Per block, classify** — `is_task = tags contains "Task" OR has status::`; `kind = if is_task then Task else Event`. The block's text comes from the block-text column (sans property lines).
4. **Per block, compute occurrences in [from, to]**:
   - **Anchor** = `scheduled::` if present (parsed as `NaiveDate`), else `deadline::`, else skip the block.
   - **Time** = the `HH:MM` suffix on the date value if any (the date string is `2026-05-22` or `2026-05-22 14:00`).
   - **Non-recurring** (`recurring::` empty or unparseable): emit a single row at `anchor` with `is_anchor=true`, if `anchor` is within `[from, to]`. Done.
   - **Recurring** (parse `recurring::` via `recurrence::parse`; if unparseable, treat as non-recurring):
     - Get `recurrence_done` (default 0).
     - Emit the anchor as `is_anchor=true` if within window (the anchor IS the current occurrence).
     - Walk forward: `let mut current = anchor; let mut done_so_far = recurrence_done;` Loop:
       - `let next = recurrence::advance(&rec, current, done_so_far);`
       - `match next { None => break, Some(d) if d > to => break, Some(d) => { done_so_far += 1; if d >= from { emit(d, is_anchor=false); } current = d; } }`
     - `advance` returns `None` when `count` is exhausted or the next would land past `until` — the loop naturally terminates on series-end.
   - **Anchor before `from`**: if the anchor itself is before `from` (e.g. a weekly task anchored 2 months ago), the loop still emits future occurrences within `[from, to]` — but the anchor itself is suppressed (not in window). Walk to current first: before emitting anything, fast-forward `current` to the first occurrence `>= from` by repeated `advance` calls (still gating `done_so_far` correctly), but mark the first emitted row `is_anchor=true` ONLY if it equals the actual block anchor date. Simpler rule: **`is_anchor=true` iff the row's date equals the block's current `scheduled::`/`deadline::` value**.
5. **`overdue`** = `occurrence_date < today` (string compare on ISO dates is correct).
6. **Sort** by `(occurrence_date, occurrence_time.unwrap_or(""), block_id)` ascending.

Pseudocode:

```rust
async fn agenda_blocks(&self, from: &str, to: &str, include_done: bool) -> Result<Vec<AgendaRow>> {
    use chrono::NaiveDate;
    use crate::recurrence::{self, advance};

    let today = chrono::Local::now().date_naive();
    let from_date = NaiveDate::parse_from_str(from, "%Y-%m-%d")?;
    let to_date = NaiveDate::parse_from_str(to, "%Y-%m-%d")?;

    // Fetch all candidate blocks via the existing query path.
    // (Exact API: mirror calendar_marks — it fetches dated blocks already.)
    let candidates = /* fetch blocks with has:deadline OR has:scheduled, optionally -status:done */;

    let mut rows: Vec<AgendaRow> = Vec::new();
    for block in candidates {
        let (anchor_date, anchor_time) = parse_anchor(&block); // scheduled:: first, then deadline::
        let is_task = block_is_task(&block);
        let kind = if is_task { AgendaRowKind::Task } else { AgendaRowKind::Event };
        let rec = block.properties.get("recurring").and_then(|s| recurrence::parse(s));
        let done_so_far_start: u32 = block.properties.get("recurrence_done")
            .and_then(|s| s.parse().ok()).unwrap_or(0);

        // Helper to push a row.
        let mut push = |date: NaiveDate, time: Option<String>, is_anchor: bool| {
            rows.push(AgendaRow {
                block_id: block.id.clone(),
                source_note_id: block.note_id.clone(),
                occurrence_date: date.format("%Y-%m-%d").to_string(),
                occurrence_time: time,
                kind,
                overdue: date < today,
                recurrence: block.properties.get("recurring").cloned(),
                is_anchor,
                text: block.text.clone(),
                status: block.properties.get("status").cloned(),
            });
        };

        match rec {
            None => {
                if anchor_date >= from_date && anchor_date <= to_date {
                    push(anchor_date, anchor_time.clone(), true);
                }
            }
            Some(rec) => {
                // Emit anchor if in window.
                if anchor_date >= from_date && anchor_date <= to_date {
                    push(anchor_date, anchor_time.clone(), true);
                }
                // Walk forward.
                let mut current = anchor_date;
                let mut done_so_far = done_so_far_start;
                loop {
                    let next = advance(&rec, current, done_so_far);
                    let next = match next {
                        None => break,
                        Some(d) if d > to_date => break,
                        Some(d) => d,
                    };
                    done_so_far += 1;
                    if next >= from_date && next != anchor_date {
                        push(next, anchor_time.clone(), false);
                    }
                    current = next;
                }
            }
        }
    }

    rows.sort_by(|a, b| {
        a.occurrence_date.cmp(&b.occurrence_date)
            .then_with(|| a.occurrence_time.cmp(&b.occurrence_time))
            .then_with(|| a.block_id.cmp(&b.block_id))
    });
    Ok(rows)
}
```

Fill in `parse_anchor`, `block_is_task`, and the candidate-fetching call against the real `SqliteIndex` API. Read the existing `calendar_marks` impl for the fetch pattern.

- [ ] **Step 5: Run tests**

Run: `cargo test -p tesela-core --lib agenda` → PASS (the three new tests + any others matching).
Run: `cargo build --release -p tesela-core` → clean.

- [ ] **Step 6: Commit**

```bash
git add crates/tesela-core/src/db/sqlite.rs
git commit -m "feat(core): agenda_blocks — query + recurrence projection in SQLite"
```

---

## Task 3: `POST /agenda` HTTP handler

**Files:**
- Create: `crates/tesela-server/src/routes/agenda.rs`
- Modify: `crates/tesela-server/src/routes/mod.rs`

Add the HTTP endpoint that calls `state.index.agenda_blocks(from, to, include_done)`.

- [ ] **Step 1: Write the failing test**

In `crates/tesela-server/src/routes/agenda.rs` (which doesn't exist yet — create it with a test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestServer; // mirror the harness used by other route tests

    #[tokio::test]
    async fn post_agenda_returns_rows_in_window() {
        let server = TestServer::fresh().await;
        server.put_note("a",
            "- weekly review\n  scheduled:: 2026-05-22\n  recurring:: weekly\n  tags:: Task\n  status:: todo\n"
        ).await;
        let resp: Vec<AgendaRow> = server.post_json(
            "/agenda",
            serde_json::json!({ "from": "2026-05-22", "to": "2026-06-12", "include_done": false }),
        ).await;
        assert_eq!(resp.len(), 4);
        assert_eq!(resp[0].occurrence_date, "2026-05-22");
        assert!(resp[0].is_anchor);
    }
}
```

Adapt `TestServer::fresh` and the `post_json` helper to whatever name the existing route tests in `crates/tesela-server/src/routes/*.rs` use — read one (e.g. `search_query.rs`) for the harness.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p tesela-server post_agenda_returns_rows` → FAIL (route undefined).

- [ ] **Step 3: Implement the handler**

In `crates/tesela-server/src/routes/agenda.rs`:

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use std::sync::Arc;
use tesela_core::query::AgendaRow;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AgendaQuery {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub include_done: bool,
}

pub async fn post_agenda(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AgendaQuery>,
) -> Result<Json<Vec<AgendaRow>>, (StatusCode, String)> {
    let rows = state
        .index
        .agenda_blocks(&body.from, &body.to, body.include_done)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}
```

(Mirror the exact `State` / `AppState` / `Json` / error-type idioms used by `search_query.rs` — read it first; if `AppState` isn't `Arc<>` there, drop the wrapper.)

- [ ] **Step 4: Register the route**

In `crates/tesela-server/src/routes/mod.rs`, add the module and route:

```rust
pub mod agenda;
```

In the router-building function (search `Router::new().route` in `routes/mod.rs` or `main.rs`):

```rust
.route("/agenda", post(agenda::post_agenda))
```

(Match the exact router-building style — some routes use `.route("/path", post(handler))`, others nest via `Router::merge`. Use the same shape as the other recent additions like `recur-bump`.)

- [ ] **Step 5: Run tests + build**

Run: `cargo test -p tesela-server agenda` → PASS.
Run: `cargo build --release -p tesela-server` → clean.

- [ ] **Step 6: Commit**

```bash
git add crates/tesela-server/src/routes/agenda.rs crates/tesela-server/src/routes/mod.rs
git commit -m "feat(server): POST /agenda — return agenda rows in a window"
```

---

## Task 4: Web — `api.getAgenda` + ambient registration + `:agenda` verb

**Files:**
- Modify: `web/src/lib/api-client.ts`
- Create: `web/src/lib/ambients/agenda/index.ts`
- Modify: `web/src/lib/renderers/register.ts`
- Modify: `web/src/lib/v4/commands.ts`

Small infrastructure: wire the API method, register the ambient renderer (component is a stub for now — Task 5 fills it in), add the `:agenda` verb.

- [ ] **Step 1: Add `getAgenda` to the API client**

In `web/src/lib/api-client.ts`, add (alongside other endpoint methods; read the file for the existing style — `recurBump` is a recent precedent):

```typescript
  getAgenda: (from: string, to: string, includeDone = false) =>
    post<AgendaRow[]>("/agenda", { from, to, include_done: includeDone }),
```

Import `AgendaRow` from `$lib/types/AgendaRow` (the file generated by `ts-rs` from Task 1's `#[derive(TS)]`; if the repo doesn't regenerate types automatically, run `cargo test -p tesela-core --lib export_bindings` to emit the `.ts` files and confirm `AgendaRow.ts` lands at `web/src/lib/types/`).

- [ ] **Step 2: Create a stub `Agenda.svelte`**

Create `web/src/lib/ambients/agenda/Agenda.svelte` as a minimal placeholder so the registration compiles:

```svelte
<script lang="ts">
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  let { onNavigate }: AmbientRendererProps = $props();
  // Task 5 fills this in. For now, render a placeholder so the ambient
  // renderer registers and `:agenda` opens *something*.
  void onNavigate;
</script>

<div class="p-4 text-sm text-muted-foreground">agenda — coming up</div>
```

- [ ] **Step 3: Create the ambient renderer**

Create `web/src/lib/ambients/agenda/index.ts` mirroring `today-in-progress/index.ts`:

```typescript
import type { AmbientRenderer } from "$lib/buffer/protocol";
import Component from "./Agenda.svelte";

const renderer: AmbientRenderer = {
  cascade: { default: Component, modes: [] },
};

export default renderer;
```

- [ ] **Step 4: Register it**

In `web/src/lib/renderers/register.ts`, follow the existing pattern (which already registers 4 ambients). Add:

```typescript
import agendaAmbient from "$lib/ambients/agenda";
// …
ambientRegistry.register("agenda", agendaAmbient);
```

Match the file's actual import/registration style — read it.

- [ ] **Step 5: Add the `:agenda` verb**

In `web/src/lib/v4/commands.ts`, add to the `AMBIENTS` array (lines ~57-62 per the prior exploration):

```typescript
  { name: "agenda", label: "Agenda", verb: "agenda", glyph: "📅" },
```

(The calendar glyph is taken — pick a distinct one if needed: `📋` / `🗓` / `✓`. Match the visual weight of the existing four.)

- [ ] **Step 6: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "agenda|api-client.ts|register.ts|v4/commands.ts"` → `0`.
Run: `cd web && pnpm build` → succeeds. Run: `pnpm test:unit` → no regressions.
Run the server (or trust the route is wired); from the browser open the command palette (`⌘K`), search "agenda", invoke — should split-open the placeholder "agenda — coming up" pane.

- [ ] **Step 7: Commit**

```bash
git add web/src/lib/api-client.ts web/src/lib/ambients/agenda web/src/lib/renderers/register.ts web/src/lib/v4/commands.ts
git commit -m "feat(web): agenda ambient registration + :agenda verb + api.getAgenda"
```

---

## Task 5: `AgendaRow.svelte` + `AgendaDay.svelte`

**Files:**
- Create: `web/src/lib/ambients/agenda/AgendaRow.svelte`
- Create: `web/src/lib/ambients/agenda/AgendaDay.svelte`

Presentation only — render a single row and a single day section. Interactions land in Task 7.

- [ ] **Step 1: Read prior art**

Read `web/src/lib/components/BlockDateRow.svelte` for the row visual style (label colors, the date/recurrence chip styling, the `formatDateMonthDay`/`formatRecurrence` imports, the `gotoNote` navigation pattern). The agenda row uses the same visual weight but a slightly different layout (icon + time + text + source pill + recurrence chip).

- [ ] **Step 2: Create `AgendaRow.svelte`**

```svelte
<script lang="ts">
  import type { AgendaRow } from "$lib/types/AgendaRow";
  import { formatDateMonthDay } from "$lib/date-format";
  import { formatRecurrence } from "$lib/recurrence-format";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";

  let { row }: { row: AgendaRow } = $props();

  const isTask = $derived(row.kind === "task");
  const isOverdue = $derived(row.overdue);
  const showCheckbox = $derived(isTask && row.is_anchor);
  // The date/icon column: deadline (⚑) when a deadline-anchored task,
  // scheduled (🕒) otherwise. Heuristic: if the row's `recurrence` is set
  // OR the source has a `scheduled::`, use 🕒; else ⚑. For v1, prefer 🕒
  // when `occurrence_time` is set (timed = scheduled).
  // The server already discriminates: if you want a clean signal, extend
  // AgendaRow with a `field: "deadline" | "scheduled"` later; for now
  // infer with: time-present → 🕒; else 🕒 (most rows are scheduled in
  // practice). Use ⚑ only on overdue tasks for visual urgency.
  const icon = $derived(isOverdue && isTask ? "⚑" : "🕒");
  const iconClass = $derived(isOverdue ? "text-[#e07b5f]" : "text-[#8fb0d4]");

  const timeOrDate = $derived(
    row.occurrence_time
      ? row.occurrence_time
      : formatDateMonthDay(row.occurrence_date),
  );
</script>

<div class="flex items-center gap-2 py-0.5 text-[13px]">
  {#if showCheckbox}
    <span
      role="checkbox"
      aria-checked={row.status === "done"}
      tabindex="0"
      class="inline-block w-3.5 h-3.5 border border-muted-foreground/60 rounded-sm cursor-pointer"
    ></span>
  {:else if isTask}
    <span class="inline-block w-3.5 h-3.5"></span>
  {:else}
    <span class="text-muted-foreground/50 text-[11px] w-3.5 text-center">·</span>
  {/if}
  <span class={iconClass}>{icon} {timeOrDate}</span>
  <span class="text-foreground/90 flex-1 truncate">{row.text}</span>
  <button
    type="button"
    class="text-[11px] text-muted-foreground/60 hover:text-foreground"
    onclick={() => gotoNote(row.source_note_id)}
  >in [[{row.source_note_id}]]</button>
  {#if row.recurrence}
    <span class="text-[11px] text-muted-foreground/50">↻ {formatRecurrence(row.recurrence)}</span>
  {/if}
</div>
```

The checkbox is presentational here (no click handler yet — Task 7 wires it). The source pill uses the same `gotoNote` pattern `BlockDateRow` uses.

- [ ] **Step 3: Create `AgendaDay.svelte`**

```svelte
<script lang="ts">
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import AgendaRow from "./AgendaRow.svelte";

  let {
    label,
    rows,
    emphasis = "normal",
  }: {
    /** Day header text — `Today · Friday, May 22` / `Mon May 25` / `Overdue`. */
    label: string;
    rows: AgendaRowT[];
    /** `overdue` tints the header; `empty` shows the empty-day placeholder. */
    emphasis?: "normal" | "overdue" | "empty";
  } = $props();
</script>

<div class="mb-3">
  <div
    class="text-[11px] font-semibold tracking-wide uppercase mb-1"
    class:text-[#e07b5f]={emphasis === "overdue"}
    class:text-muted-foreground/40={emphasis === "empty"}
    class:text-muted-foreground/70={emphasis === "normal"}
  >{label}</div>
  {#if emphasis === "empty"}
    <!-- nothing -->
  {:else}
    {#each rows as row (row.block_id + ":" + row.occurrence_date)}
      <AgendaRow {row} />
    {/each}
  {/if}
</div>
```

- [ ] **Step 4: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "Agenda(Row|Day).svelte"` → `0`.
Run: `cd web && pnpm build` → succeeds.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/ambients/agenda/AgendaRow.svelte web/src/lib/ambients/agenda/AgendaDay.svelte
git commit -m "feat(web): agenda — row + day presentation components"
```

---

## Task 6: `Agenda.svelte` — fetch + window + scroll + toggle

**Files:**
- Modify: `web/src/lib/ambients/agenda/Agenda.svelte`

The top-level: state, fetch, group-into-days, render Overdue + Today + future days, hide-done toggle, infinite-scroll-forward.

- [ ] **Step 1: Replace the stub**

Replace the stub from Task 4 with the real component. Read the file then rewrite:

```svelte
<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import AgendaDay from "./AgendaDay.svelte";

  let { onNavigate }: AmbientRendererProps = $props();
  void onNavigate;

  // Window state — initial fetch is today → today + 60d; "load more"
  // bumps the upper bound.
  function isoDate(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  }
  const todayIso = isoDate(new Date());
  let upperOffset = $state(60); // days past today
  let includeDone = $state(false);

  const upperIso = $derived.by(() => {
    const d = new Date();
    d.setDate(d.getDate() + upperOffset);
    return isoDate(d);
  });

  const q = createQuery(() => ({
    queryKey: ["agenda", { from: todayIso, to: upperIso, includeDone }] as const,
    queryFn: () => api.getAgenda(todayIso, upperIso, includeDone),
  }));

  const rows = $derived((q.data ?? []) as AgendaRowT[]);

  // Split into Overdue + per-day buckets across [today, upperIso].
  const buckets = $derived.by(() => {
    const overdue: AgendaRowT[] = [];
    const byDay = new Map<string, AgendaRowT[]>();
    for (const r of rows) {
      if (r.overdue) overdue.push(r);
      else (byDay.get(r.occurrence_date) ?? byDay.set(r.occurrence_date, []).get(r.occurrence_date)!).push(r);
    }
    // Walk the window day-by-day so empty days render as placeholders.
    const days: { iso: string; label: string; rows: AgendaRowT[] }[] = [];
    const start = new Date();
    for (let i = 0; i <= upperOffset; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      const iso = isoDate(d);
      const dayRows = byDay.get(iso) ?? [];
      const label =
        i === 0 ? `Today · ${formatDayHeader(d)}`
        : i === 1 ? `Tomorrow · ${formatDayHeader(d)}`
        : formatDayHeader(d);
      days.push({ iso, label, rows: dayRows });
    }
    return { overdue, days };
  });

  function formatDayHeader(d: Date): string {
    return d.toLocaleDateString("en-US", { weekday: "long", month: "short", day: "numeric" });
  }

  // Infinite scroll — when the sentinel is near, extend the window.
  let sentinel = $state<HTMLElement | undefined>();
  $effect(() => {
    const node = sentinel;
    if (!node) return;
    const obs = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (e.isIntersecting) upperOffset = upperOffset + 60;
      }
    }, { rootMargin: "200px" });
    obs.observe(node);
    return () => obs.disconnect();
  });
</script>

<div class="flex flex-col h-full min-h-0 overflow-auto px-4 py-3">
  <header class="flex items-center justify-between mb-3 text-[12px]">
    <div class="font-semibold">📅 Agenda</div>
    <label class="flex items-center gap-2 cursor-pointer text-muted-foreground">
      <input type="checkbox" bind:checked={includeDone} class="cursor-pointer" />
      <span>show done</span>
    </label>
  </header>

  {#if q.isLoading}
    <div class="text-muted-foreground/60 text-[12px]">loading…</div>
  {:else}
    {#if buckets.overdue.length > 0}
      <AgendaDay label="Overdue" rows={buckets.overdue} emphasis="overdue" />
    {/if}
    {#each buckets.days as day (day.iso)}
      {#if day.rows.length > 0}
        <AgendaDay label={day.label} rows={day.rows} />
      {:else}
        <AgendaDay label={`${day.label} — empty`} rows={[]} emphasis="empty" />
      {/if}
    {/each}
    <div bind:this={sentinel} class="h-px"></div>
  {/if}
</div>
```

- [ ] **Step 2: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -c "Agenda.svelte"` → `0`. Run: `pnpm build` → succeeds. Run: `pnpm test:unit` → no regressions. Open the agenda from `⌘K` → see today + the scrollable window populated by the running server.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/ambients/agenda/Agenda.svelte
git commit -m "feat(web): agenda — fetch + day buckets + infinite scroll + hide-done toggle"
```

---

## Task 7: Interactions — mark done, reschedule, skip

**Files:**
- Modify: `web/src/lib/ambients/agenda/AgendaRow.svelte`
- Modify: `web/src/lib/ambients/agenda/Agenda.svelte` (for the save callback wiring)

Wire the three interactions on the current-anchor rows. Read `BlockDateRow.svelte` and `recurrence-actions.ts` first — both established patterns this task reuses.

- [ ] **Step 1: Mark-done wiring**

In `AgendaRow.svelte`: the checkbox `<span role="checkbox">` becomes a real `<button>` with a click handler. The agenda doesn't own the block's text, so the cleanest path: hit `api.updateNote` after fetching the source note, applying `upsertBlockProperty(noteContent, "status", "done")` *targeting the specific block by id* (the existing `BlockDateRow` writes the block's `raw_text` and saves via `BlockOutliner.handleBlockChange`; the agenda doesn't have that handle).

The simplest correct path: PUT a note-level update is too coarse if the same note has unrelated edits in flight. Use the server's existing block-property update endpoint if there is one — `rg -n "block.property|/blocks/" crates/tesela-server/src/routes/` to find it. If a block-property endpoint exists (highly likely given the recurrence-bump endpoint), call it; on the response, refetch the agenda query (`queryClient.invalidateQueries({ queryKey: ["agenda"] })`).

If there is **no** existing block-property endpoint:
- Add a `POST /blocks/:id/set-property` server endpoint (small handler — fetches the note containing the block, applies `upsertBlockProperty` to the block's text, saves the note via the existing save path, returning the updated note). Wire `api.setBlockProperty(blockId, key, value)` in `api-client.ts`.
- This is a small server addition; if its absence is the case, do it here in Task 7 rather than expanding scope earlier.

In `AgendaRow.svelte`, on checkbox click (current-anchor task rows only):
```typescript
async function markDone() {
  if (row.status === "done") return;
  await api.setBlockProperty(row.block_id, "status", "done"); // or the existing endpoint name
  await queryClient.invalidateQueries({ queryKey: ["agenda"] });
}
```

Import `queryClient` from `$lib/app-query-client.svelte` (the pattern the skip verb uses).

- [ ] **Step 2: Reschedule wiring**

Click on the date span opens the existing standalone `DatePicker` component (the same one `BlockDateRow` opens). On commit, write the new date to the block — same `setBlockProperty(row.block_id, "scheduled", isoDate)` call. Then invalidate the agenda query. The DatePicker reuse: render `<DatePicker>` conditionally with `{#if editing}` in the row, pre-filled with the current `occurrence_date`. Reuse the exact open/close pattern `BlockDateRow.svelte` uses (read it).

Restriction (per spec §4): only the current-anchor row's date is editable; projected future rows are read-only. The date span is a button only when `row.is_anchor`.

- [ ] **Step 3: Skip wiring**

Add a small `⏭` button next to the recurrence chip, visible only when `row.is_anchor && row.recurrence`. On click: `await skipRecurrence(row.block_id)` (the existing helper) then invalidate the agenda query.

```typescript
import { skipRecurrence } from "$lib/recurrence-actions";
// …
{#if row.is_anchor && row.recurrence}
  <button
    type="button"
    class="text-[11px] text-muted-foreground/60 hover:text-foreground"
    onclick={async () => { await skipRecurrence(row.block_id); queryClient.invalidateQueries({ queryKey: ["agenda"] }); }}
    title="Skip to next occurrence"
  >⏭</button>
{/if}
```

- [ ] **Step 4: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "Agenda(Row|.svelte)"` → `0`.
Run: `pnpm build` → succeeds. Run: `pnpm test:unit` → no regressions.
Manually exercise: mark a task done → it disappears (with `show done` off); reschedule a task to a different day → it moves; skip a recurring task → the anchor advances.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/ambients/agenda/AgendaRow.svelte web/src/lib/ambients/agenda/Agenda.svelte web/src/lib/api-client.ts crates/tesela-server/src/routes/
git commit -m "feat(web): agenda — inline mark-done, reschedule, skip"
```

(Stage only files you actually changed — the server-route file is included only if you had to add a new endpoint in Step 1.)

---

## Self-Review

**Spec coverage (design spec §1–6):**
- §1 ambient buffer — Task 4 (registration + verb).
- §2 data + projection — Tasks 1–3 (types, SQLite projection, HTTP endpoint).
- §3 component layout (header, day buckets, row anatomy) — Tasks 5 + 6.
- §4 interactions (mark done, reschedule, skip — all current-anchor only) — Task 7. The `is_anchor` gate ships across the chain: server emits it (Task 2), row reads it for the checkbox/date-edit/skip affordances (Tasks 5 + 7).
- §5 component decomposition — Tasks 5 (Row/Day) + 6 (Agenda).
- §6 out of scope — agenda-side past-days, per-occurrence overrides, drag-to-reschedule, bulk ops, and iOS aren't in any task. Correct.

**Type consistency:** `AgendaRow` is defined in Task 1 with fields `{ block_id, source_note_id, occurrence_date, occurrence_time, kind, overdue, recurrence, is_anchor, text, status }`. Tasks 4 (api), 5 (Row/Day), 6 (Agenda), and 7 (interactions) all consume those exact field names. `agenda_blocks(from, to, include_done)` is the single signature across Tasks 1, 2, 3, 4.

**Placeholder scan:** Task 2's "fetch candidate blocks" step deliberately points at the existing `calendar_marks` impl to mirror (specific local conventions); Task 7 instructs `rg` to find any existing block-property endpoint before adding one. All code-bearing steps carry real code.

**Ordering:** Tasks 1→2→3 build the server up the dependency chain. Task 4 is the smallest web step; Tasks 5→6→7 build the UI presentation → fetch → interactions in order. Each task is independently committable and reviewable.
