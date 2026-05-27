//! Spike item 4 — Loro move-op semantics on the scenarios that matter
//! to Tesela.
//!
//! Scenario A (the canonical "move + concurrent edit" case):
//!   Device 1 moves block A under block B.
//!   Device 2 concurrently edits block A's text.
//!   Merge. Expect: both edits visible — A is under B AND has device 2's text.
//!
//! Scenario B (move cycle):
//!   Device 1 moves A under B.
//!   Device 2 moves B under A.
//!   Merge. Expect: one wins deterministically (Loro picks by HLC),
//!   the cycle-creating one is dropped without crashing.
//!
//! Run: `cargo run -p tesela-loro-spike --bin spike-move-parity`

use loro::{LoroDoc, LoroTree};

fn scenario_move_and_edit() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Scenario A: move + concurrent text edit ===");
    // Create the initial state with two top-level nodes (A, B) and sync
    // it to both devices.
    let origin = LoroDoc::new();
    let tree: LoroTree = origin.get_tree("blocks");
    let a = tree.create(None)?;
    let b = tree.create(None)?;
    // Each node carries a text meta entry — this is the "block text".
    tree.get_meta(a)?.insert("text", "A original")?;
    tree.get_meta(b)?.insert("text", "B original")?;
    origin.commit();

    // Fork: device 1 and device 2 both load the origin state.
    let d1 = LoroDoc::new();
    d1.import(&origin.export(loro::ExportMode::all_updates())?)?;
    let d2 = LoroDoc::new();
    d2.import(&origin.export(loro::ExportMode::all_updates())?)?;

    // Device 1: move A under B.
    let d1_tree: LoroTree = d1.get_tree("blocks");
    d1_tree.mov(a, b)?;
    d1.commit();

    // Device 2: edit A's text concurrently.
    let d2_tree: LoroTree = d2.get_tree("blocks");
    d2_tree.get_meta(a)?.insert("text", "A EDITED by device 2")?;
    d2.commit();

    // Merge both into a third doc.
    let merged = LoroDoc::new();
    merged.import(&d1.export(loro::ExportMode::all_updates())?)?;
    merged.import(&d2.export(loro::ExportMode::all_updates())?)?;
    let merged_tree: LoroTree = merged.get_tree("blocks");

    let a_parent = merged_tree.parent(a).map(|p| format!("{:?}", p));
    let a_meta = merged_tree.get_meta(a)?.get("text").map(|v| format!("{:?}", v));
    println!("  A's parent after merge:  {:?} (expected: Some({:?}))", a_parent, b);
    println!("  A's text after merge:    {:?} (expected: contains 'EDITED by device 2')", a_meta);

    let ok = a_parent.is_some()
        && a_meta
            .as_ref()
            .map(|s| s.contains("EDITED by device 2"))
            .unwrap_or(false);
    println!("  Scenario A verdict: {}", if ok { "GREEN" } else { "FAIL" });
    Ok(())
}

fn scenario_move_cycle() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Scenario B: concurrent move cycle (A→B and B→A) ===");
    let origin = LoroDoc::new();
    let tree: LoroTree = origin.get_tree("blocks");
    let a = tree.create(None)?;
    let b = tree.create(None)?;
    origin.commit();

    let d1 = LoroDoc::new();
    d1.import(&origin.export(loro::ExportMode::all_updates())?)?;
    let d2 = LoroDoc::new();
    d2.import(&origin.export(loro::ExportMode::all_updates())?)?;

    // Device 1: move A under B.
    d1.get_tree("blocks").mov(a, b)?;
    d1.commit();

    // Device 2: move B under A.
    d2.get_tree("blocks").mov(b, a)?;
    d2.commit();

    let merged = LoroDoc::new();
    merged.import(&d1.export(loro::ExportMode::all_updates())?)?;
    let import_result = merged.import(&d2.export(loro::ExportMode::all_updates())?);
    let crashed = import_result.is_err();
    println!("  Second import errored?  {}", crashed);

    let merged_tree: LoroTree = merged.get_tree("blocks");
    let a_parent = merged_tree.parent(a).map(|p| format!("{:?}", p));
    let b_parent = merged_tree.parent(b).map(|p| format!("{:?}", p));
    println!("  A's parent after merge: {:?}", a_parent);
    println!("  B's parent after merge: {:?}", b_parent);
    println!("  (Expectation: exactly ONE of the two moves wins, the other is dropped — no cycle, no crash)");

    let no_cycle = !(a_parent.is_some() && b_parent.is_some());
    println!(
        "  Scenario B verdict: {}",
        if !crashed && no_cycle { "GREEN" } else { "FAIL" }
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    scenario_move_and_edit()?;
    scenario_move_cycle()?;
    Ok(())
}
