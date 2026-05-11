# Performance regression harness

Phase 14 added a criterion-based bench harness so we catch scaling
regressions *before* they ship. This doc is the developer-facing
entry point.

## Running benches

All benches live in each crate's `benches/` directory and use
`harness = false` so they're driven by criterion directly.

```sh
# Run every bench
cargo bench --workspace

# Run benches in one crate
cargo bench -p tesela-core
cargo bench -p tesela-backup
cargo bench -p tesela-server

# Run a single named bench file
cargo bench --bench list_notes -p tesela-core

# Quick-iteration mode (lower sample count, ~5x faster, less precise)
cargo bench --bench list_notes -p tesela-core -- --quick
```

Criterion writes results to `target/criterion/`. The HTML report
lives at `target/criterion/report/index.html` — open it in a browser
to see history graphs and percentile distributions per bench.

## Establishing + comparing baselines

Criterion has built-in baseline support. The dev loop:

1. After every merge to `main` (or any "this is the new floor"
   moment), capture a baseline:
   ```sh
   cargo bench --workspace -- --save-baseline main
   ```
2. On a feature branch, run the same benches again. Criterion
   automatically diffs vs the saved baseline and reports a
   `change: [-15% -8% +2%]` line per bench.
3. Anything more than ~10% slower is worth investigating before
   shipping. The harness is informational — no automatic gating
   (yet).

## Frontend smoke suite

The web perf smoke suite drives the real Svelte app through Playwright
against a generated `tesela-fixtures::medium()` mosaic:

```sh
pnpm --dir web test:perf
```

The runner creates a temp mosaic with `cargo run -p tesela-fixtures-cli
-- --preset medium --out <path>`, starts `tesela-server` on
`127.0.0.1:7474`, starts Vite on `127.0.0.1:4174`, and records JSONL
timings at `web/test-results/perf-timings.jsonl`. The smoke coverage is:

- `/p/dailies` first paint to the fifth day section (<1.5s)
- rail navigation to the Tasks kanban first card (<800ms)
- command palette open and first result render (<300ms)
- Settings → Mosaic → Logseq create/import plan preview (<5s)

## Adding a new bench

1. Pick the crate that owns the function under test.
2. Add a file under `<crate>/benches/<name>.rs` and register it in
   `<crate>/Cargo.toml`:
   ```toml
   [[bench]]
   name = "my_new_bench"
   harness = false
   ```
3. Use `tesela_fixtures::MosaicBuilder` (or one of the `tiny()` /
   `medium()` / `large()` presets) to build a synthetic mosaic in
   the bench setup. Same seed ⇒ deterministic output, so the bench
   is reproducible.
4. Use `criterion::iter_batched` to keep setup cost out of the timer:
   ```rust
   b.iter_batched(
       || MosaicBuilder::medium().build().unwrap(),
       |mosaic| {
           rt.block_on(async {
               // … exercise the thing …
           })
       },
       criterion::BatchSize::SmallInput,
   );
   ```

## What's covered today

| Bench | Crate | Bytes under test |
|---|---|---|
| `index` | `tesela-core` | `Indexer::initial_index` on 30 + 500 note mosaics |
| `list_notes` | `tesela-core` | `NoteStore::list` at varied limits (30 / 60 / 500 / MAX) + tag-filtered |
| `typed_blocks` | `tesela-core` | `SqliteIndex::get_typed_blocks("Task")` on 500 notes / 200 tasks (unbounded SELECT path) |
| `backup` | `tesela-backup` | full `backup()` with manifest validation + GFS prune on 30 + 500 notes |
| `http` | `tesela-server` | end-to-end HTTP: `/notes`, `/types/Task/blocks` against a spawned server |

## Out of scope (yet)

- **Blocking perf gates**: `.github/workflows/perf.yml` runs Rust
  benches and the frontend smoke suite informationally. It comments on
  PRs only when a Rust benchmark regresses by more than 10%.
- **Memory profiling**: wall-clock first.
