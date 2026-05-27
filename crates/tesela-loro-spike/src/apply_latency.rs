//! Spike item 3 — Loro apply-changes latency on a representative op batch.
//!
//! Builds a source doc that has executed 100 mixed ops (inserts, edits,
//! moves, deletes), exports its updates, then times `doc.import(updates)`
//! on a fresh destination doc. Reports median over N runs.
//!
//! Run: `cargo run -p tesela-loro-spike --release --bin spike-apply-latency`

use loro::{ExportMode, LoroDoc, LoroMovableList};
use std::time::Instant;

const RUNS: usize = 10;
const OP_COUNT: usize = 100;

fn build_source_doc() -> LoroDoc {
    let doc = LoroDoc::new();
    let list: LoroMovableList = doc.get_movable_list("blocks");

    // 60 inserts
    for i in 0..60 {
        list.insert(
            list.len(),
            format!("Block {} with some realistic text content that's not too short", i)
                .as_str(),
        )
        .unwrap();
    }
    // 25 edits — replace some blocks (LoroMovableList: delete + insert)
    for i in 0..25 {
        let idx = (i * 2) % (list.len());
        list.delete(idx, 1).unwrap();
        list.insert(idx, format!("Edited block {}", i).as_str()).unwrap();
    }
    // 10 moves
    for i in 0..10 {
        let from = i % list.len();
        let to = (from + 5) % list.len();
        if from != to {
            list.mov(from, to).unwrap();
        }
    }
    // 5 deletes
    for _ in 0..5 {
        if list.len() > 1 {
            list.delete(0, 1).unwrap();
        }
    }
    doc.commit();
    doc
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = build_source_doc();
    let updates = source.export(ExportMode::all_updates())?;
    println!(
        "Built source doc with ~{} ops; updates blob = {} bytes",
        OP_COUNT,
        updates.len()
    );

    let mut timings = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let target = LoroDoc::new();
        let start = Instant::now();
        target.import(&updates)?;
        timings.push(start.elapsed());
    }
    timings.sort();
    let median = timings[RUNS / 2];
    let min = timings[0];
    let max = timings[RUNS - 1];

    println!(
        "import() over {} runs: median {:?}, min {:?}, max {:?}",
        RUNS, median, min, max
    );
    let per_op_us = median.as_micros() as f64 / OP_COUNT as f64;
    println!("≈ {:.1} µs per op (median)", per_op_us);

    // Verdict gates per the spike spec (per-batch totals):
    // Green: < 100 ms
    // Yellow: 100 ms – 1 s
    // Red: > 1 s
    let verdict = if median.as_millis() > 1_000 {
        "RED — > 1 s for 100 ops"
    } else if median.as_millis() > 100 {
        "YELLOW — 100 ms – 1 s for 100 ops"
    } else {
        "GREEN — < 100 ms for 100 ops"
    };
    println!("\nVerdict: {}", verdict);
    Ok(())
}
