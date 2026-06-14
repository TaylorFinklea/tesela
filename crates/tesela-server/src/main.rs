//! Thin `tesela-server` binary entry point. All boot logic lives in the
//! library (`tesela_server::serve`); this just parses CLI args, sets up
//! tracing + the parent-death watchdog, and runs the server to a SIGINT /
//! SIGTERM. The desktop Tauri shell links the same library and calls
//! `serve` in-process instead of spawning this binary (L4).

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use tesela_server::{serve, spawn_parent_death_watchdog, wait_for_shutdown_signal, ServeConfig};

#[derive(Debug, Parser)]
#[command(
    name = "tesela-server",
    about = "Tesela HTTP server (notes API, sync daemon, WebSocket)"
)]
struct Args {
    /// Override the mosaic directory. Takes precedence over the
    /// TESELA_DEFAULT_MOSAIC env var, the cwd-walk lookup, and the
    /// user's saved config.
    #[arg(long, value_name = "PATH")]
    mosaic: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Desktop child-spawn embed (legacy path): exit if our parent disappears,
    // so an orphaned server never becomes a second writer. No-op unless
    // TESELA_EXIT_WITH_PARENT is set (the in-process embed never sets it).
    spawn_parent_death_watchdog();

    let args = Args::parse();
    let config = ServeConfig::resolve(args.mosaic)?;
    serve(config, wait_for_shutdown_signal(), |_| {}).await
}
