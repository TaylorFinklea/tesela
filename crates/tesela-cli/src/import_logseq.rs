//! Standalone Logseq import through the mosaic's locked Loro engine.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tesela_core::import_logseq::{
    apply_plan_with_writer, build_plan, summarize, ApplyDecisions, ApplyOutcome,
};
use tesela_sync::EngineImportNoteWriter;

use crate::mosaic_notes::open_locked_engine;

pub async fn run(mosaic: &Path, source: PathBuf, dry_run: bool) -> Result<()> {
    let plan = build_plan(&source, mosaic).context("plan logseq import")?;
    let counts = summarize(&plan);
    if dry_run {
        println!("Dry run complete:");
    } else {
        println!("Import complete:");
    }
    println!("  Would import: {}", counts.new_imports);
    println!("  Unchanged (idempotent): {}", counts.unchanged);
    println!("  Conflicts: {}", counts.conflicts);
    println!("  Hard-skipped: {}", counts.hard_skips);
    if dry_run {
        return Ok(());
    }

    let (_lock, engine) = open_locked_engine(mosaic).await?;
    let mut writer = EngineImportNoteWriter::new(&engine);
    let outcome = apply_plan_with_writer(&plan, &ApplyDecisions::default(), mosaic, &mut writer)
        .await
        .context("apply logseq import through engine")?;
    println!("  Imported: {}", outcome.imported);
    println!("  Overwritten: {}", outcome.overwritten);
    println!("  Renamed: {}", outcome.renamed);
    println!("  Skipped: {}", outcome.skipped);
    println!("  Assets copied: {}", outcome.assets_copied);
    if !outcome.errors.is_empty() {
        println!("  Errors: {}", outcome.errors.len());
        for error in &outcome.errors {
            println!("    {error}");
        }
    }
    ensure_outcome_succeeded(&outcome)
}

fn ensure_outcome_succeeded(outcome: &ApplyOutcome) -> Result<()> {
    match outcome.errors.len() {
        0 => Ok(()),
        1 => anyhow::bail!("1 note write failed during Logseq import"),
        count => anyhow::bail!("{count} note writes failed during Logseq import"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_note_apply_errors_fail_the_cli_result() {
        let outcome = ApplyOutcome {
            errors: vec!["snapshot write failed".to_string()],
            ..ApplyOutcome::default()
        };

        let error = ensure_outcome_succeeded(&outcome).unwrap_err();
        assert!(error.to_string().contains("1 note write failed"));
    }
}
