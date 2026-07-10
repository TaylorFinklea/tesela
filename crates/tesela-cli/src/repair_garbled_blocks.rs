//! One-shot repair (tesela-49d): collapse residual disjoint-lineage TWIN blocks
//! (the same `block_id` living on more than one live Loro tree node — the
//! residue of pre-fix disjoint authoring) to a single deterministic winner.
//!
//! Since the tesela-y11 fix, twins also self-heal on the next relay round (the
//! apply path runs the same resolution). This command is the OFFLINE / FORCE
//! path for residue already sitting in the mosaic: dry-run by default (report
//! what would collapse), `--apply` to write.
//!
//! NOT handled: a pre-collapsed UNION *concatenation* on a SINGLE node (e.g. two
//! runs merged into one block's text). That is not a twin and can't be
//! auto-split without knowing the original boundary — edit those manually.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use tesela_sync::{Hlc, LoroEngine};

use crate::backfill_task::{acquire_mosaic_lock, load_device_id};

fn hex16(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// CLI entry: lock the mosaic (refuse while the server/desktop holds it), open
/// the Loro engine over its snapshots, scan for disjoint twins, report; on
/// `--apply` collapse each to the deterministic winner and persist.
pub async fn run(mosaic: &Path, apply: bool) -> Result<()> {
    let _lock = acquire_mosaic_lock(mosaic).context(
        "could not lock the mosaic — is tesela-server (or the desktop app) running on it? \
         Stop it before running repair-garbled-blocks (single-writer).",
    )?;

    let device = load_device_id(mosaic);
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let hlc = Arc::new(Hlc::new(device));
    let engine = LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir))
        .await
        .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;

    let twins = engine.scan_disjoint_twins().await;
    if twins.is_empty() {
        println!(
            "repair-garbled-blocks: no disjoint-lineage twin blocks found — nothing to repair."
        );
        println!(
            "(A pre-collapsed UNION concatenation on a single block is not a twin and can't be \
             auto-split — edit any such block manually.)"
        );
        return Ok(());
    }

    println!(
        "repair-garbled-blocks: found {} twin block(s) across the mosaic:",
        twins.len()
    );
    for (note_id, bid, texts) in &twins {
        println!("  note {} block {}", hex16(note_id), bid);
        for t in texts {
            println!("      candidate: {t:?}");
        }
    }

    if !apply {
        println!(
            "\nDRY-RUN — re-run with --apply to collapse each twin to the deterministic winner \
             (global-max TreeID; the SAME rule the live sync uses). Back up the \
             mosaic first (cp -r) if you want a rollback."
        );
        return Ok(());
    }

    let healed = engine.heal_disjoint_twins().await;
    println!(
        "\nrepair-garbled-blocks: collapsed {} twin block(s):",
        healed.len()
    );
    for (note_id, bid) in &healed {
        println!("  healed note {} block {}", hex16(note_id), bid);
    }
    println!(
        "Done — snapshots + materialized notes updated. Re-run to confirm idempotent (it should \
         then find nothing)."
    );
    Ok(())
}
