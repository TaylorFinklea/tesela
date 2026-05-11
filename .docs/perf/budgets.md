# Performance budgets

Headline metrics that should hold on a **500-note synthetic mosaic**
(matches the user's real Logseq import at 493 notes).

The numbers are *targets*, not hard test assertions — criterion
handles drift detection via baselines. If you blow a budget by more
than ~10%, investigate before shipping.

| Path | Budget | Where it's measured |
|---|---|---|
| Server startup `initial_index` (500 notes) | < 500ms | `tesela-core/benches/index.rs::initial_index_at_scale[medium_500]` |
| `NoteStore::list` (limit=60, tag=daily) | < 50ms | `tesela-core/benches/list_notes.rs::list_at_varied_limits[60]` |
| `SqliteIndex::get_typed_blocks("Task")` (500 notes, 200 tasks) | < 250ms | `tesela-core/benches/typed_blocks.rs` |
| `backup()` w/ validate (500 notes) | < 1.5s | `tesela-backup/benches/backup.rs::full_validate[medium_500]` |
| HTTP `GET /notes?tag=daily&limit=60` p95 | < 100ms | `tesela-server/benches/http.rs::list_notes_daily[60]` |
| HTTP `GET /types/Task/blocks` (500 notes / 200 tasks) | < 300ms | `tesela-server/benches/http.rs::types_task_blocks` |
| `MosaicBuilder::medium().build()` | < 2s | implicit — setup cost should be ≤ work cost |

## Why these numbers

Tesela's daily-driver workflow assumes Dailies, search, and task
queries all feel instant. 100ms is the upper bound for "feels
instant" on a local-only app; anything past ~250ms reads as "this
froze for a moment." Backup is a background op so 1.5s is fine.

## How to revise

Open this file, change the number, commit. The number is the policy;
criterion is the enforcement. If a regression PR justifies relaxing
a budget (e.g., trading speed for correctness on a security fix), say
so in the commit message and update the table.

## Real-world calibration

User's actual Logseq mosaic (used as the target for these numbers):

- 493 notes
- 33,412 lines across all bodies
- 209 MB on disk (mostly attachments)
- Avg note body: ~68 lines
- Max single note: 4,163 lines (long reference page)

The `MosaicBuilder::medium()` preset (420 dailies + 80 pages + 200
tasks) is shaped to match this baseline.
