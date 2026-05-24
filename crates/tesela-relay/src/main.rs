//! `tesela-relay` — the self-hosted reference implementation of the
//! Tesela sync relay protocol. See `.docs/ai/phases/2026-05-24-relay-
//! protocol-design.md` for the spec; the Cloudflare Worker port is a
//! separate deliverable speaking the same wire format and passing the
//! same conformance suite.
//!
//! ## What this binary does
//!
//! - Listens on `--bind` (default `0.0.0.0:8484`).
//! - Stores per-group FIFO of opaque AEAD-sealed envelopes in SQLite
//!   at `--db` (default `./relay.sqlite`).
//! - Authenticates requests via per-group HMAC using each group's
//!   stored `auth_key` (derived deterministically on every client from
//!   `group_key` via HKDF; see spec for the derivation).
//! - Lets the operator nuke a hijacked group registration via
//!   `DELETE /admin/groups/{id}/register` gated by `--admin-token`.
//!
//! ## What this binary does NOT do
//!
//! - Decrypt anything. Payloads are opaque from the relay's
//!   perspective; only group-key holders (the user's devices) can
//!   read them.
//! - Account systems, billing, multi-tenancy beyond namespacing by
//!   randomly-generated `group_id`. One deployment = one trust
//!   surface (operator).
//! - Push delivery / WebSocket. Devices poll. APNs push proxy is a
//!   separate future deliverable.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use axum::{routing::get, Router};
use clap::Parser;
use tracing::info;

mod handlers;
mod state;
mod store;

use crate::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "tesela-relay", about, long_about = None)]
struct Args {
    /// Bind address (e.g. `0.0.0.0:8484`).
    #[arg(long, env = "TESELA_RELAY_BIND", default_value = "0.0.0.0:8484")]
    bind: SocketAddr,

    /// Path to the SQLite store. Created on first run.
    #[arg(long, env = "TESELA_RELAY_DB", default_value = "./relay.sqlite")]
    db: PathBuf,

    /// Admin token for `DELETE /admin/groups/{id}/register`. If unset,
    /// admin endpoints are disabled — operators who want to be able
    /// to recover from hijack squatting must set this. Pick a long
    /// random string.
    #[arg(long, env = "TESELA_RELAY_ADMIN_TOKEN")]
    admin_token: Option<String>,

    /// Maximum body size (bytes) accepted on PUT /ops. Spec recommends
    /// 1 MiB; anything larger should be split into multiple envelopes
    /// by the producing client.
    #[arg(long, env = "TESELA_RELAY_MAX_BODY", default_value_t = 1_048_576)]
    max_body: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let state = AppState::open(&args).await?;
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    info!("tesela-relay listening on http://{}", args.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::health))
        // Handlers are stubbed for stage 2a; subsequent stages fill them in.
        .with_state(state)
}
