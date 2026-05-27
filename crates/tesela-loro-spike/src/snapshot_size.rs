//! Spike item 2 — Loro snapshot size vs current on-disk file size.
//!
//! Reads a real Tesela note (today's daily by default), parses it into a
//! NoteTree via the existing `tesela_core::note_tree::parse_note`, builds an
//! equivalent Loro doc with one movable list of blocks, exports the
//! snapshot, prints the size comparison.
//!
//! Run: `cargo run -p tesela-loro-spike --bin spike-snapshot-size -- <path>`

use loro::{ExportMode, LoroDoc, LoroMovableList};
use std::env;
use std::path::PathBuf;
use tesela_core::note_tree::parse_note;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = env::args().nth(1).unwrap_or_else(|| {
        let home = env::var("HOME").unwrap_or_default();
        format!(
            "{}/Library/Application Support/tesela/logseq/notes/2026-05-27.md",
            home
        )
    });
    let path = PathBuf::from(&arg);
    let content = std::fs::read_to_string(&path)?;
    let on_disk_bytes = content.len();

    let tree = parse_note(&content);
    println!(
        "Parsed note: {} blocks across {} on-disk bytes",
        tree.blocks.len(),
        on_disk_bytes
    );

    // Build the Loro doc: one movable list "blocks" holding text entries.
    // Each block becomes a single text element with its own id (block uuid)
    // stored alongside via the list's element ids. The movable list's
    // element-id mechanism is exactly what we want — moves preserve
    // identity, deletes target the id, inserts mint a new one.
    let doc = LoroDoc::new();
    let list: LoroMovableList = doc.get_movable_list("blocks");
    for block in &tree.blocks {
        // Use the block's first-line text as the list element.
        list.insert(list.len(), block.text.as_str())?;
    }
    doc.commit();

    let snapshot = doc.export(ExportMode::Snapshot)?;
    let update_all = doc.export(ExportMode::all_updates())?;

    println!("Loro snapshot bytes:    {}", snapshot.len());
    println!("Loro all-updates bytes: {}", update_all.len());
    println!("On-disk markdown bytes: {}", on_disk_bytes);

    let ratio = snapshot.len() as f64 / on_disk_bytes.max(1) as f64;
    println!("Snapshot / on-disk ratio: {:.2}x", ratio);

    // Verdict gates per the spike spec:
    // Green: ratio ≤ 5x
    // Yellow: 5x – 20x
    // Red: > 20x or > 100 KB total
    let verdict = if snapshot.len() > 100_000 {
        "RED — exceeded 100 KB absolute"
    } else if ratio > 20.0 {
        "RED — ratio > 20x"
    } else if ratio > 5.0 {
        "YELLOW — ratio between 5x and 20x"
    } else {
        "GREEN — ratio ≤ 5x"
    };
    println!("\nVerdict: {}", verdict);
    Ok(())
}
