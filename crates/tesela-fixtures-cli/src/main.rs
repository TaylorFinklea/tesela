use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(about = "Materialize synthetic Tesela mosaics for tests and perf harnesses")]
struct Args {
    /// Directory where the generated mosaic should be written.
    #[arg(long)]
    out: PathBuf,

    /// Fixture size preset.
    #[arg(long, value_enum, default_value_t = Preset::Medium)]
    preset: Preset,

    /// Deterministic RNG seed override.
    #[arg(long)]
    seed: Option<u64>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Preset {
    Tiny,
    Medium,
    Large,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.out.exists() && args.out.read_dir()?.next().is_some() {
        bail!("--out must be empty or absent: {}", args.out.display());
    }

    let mut builder = match args.preset {
        Preset::Tiny => tesela_fixtures::tiny(),
        Preset::Medium => tesela_fixtures::medium(),
        Preset::Large => tesela_fixtures::large(),
    };
    if let Some(seed) = args.seed {
        builder = builder.seed(seed);
    }

    let stats = builder.build_at(&args.out)?;
    eprintln!(
        "generated {} notes, {} blocks, {} tasks at {}",
        stats.notes,
        stats.blocks,
        stats.tasks,
        args.out.display()
    );
    println!("{}", args.out.display());
    Ok(())
}
