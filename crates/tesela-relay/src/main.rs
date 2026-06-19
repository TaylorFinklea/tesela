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
//! - WebSocket. Devices poll. (APNs silent-push is now OPTIONAL — set
//!   `APNS_*` to nudge suspended iOS devices to pull on each deposit;
//!   unset → poll-only, exactly as before. The push carries no content.)

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::info;

use tesela_relay::{router, AppState};

#[derive(Parser, Debug)]
#[command(name = "tesela-relay", about, long_about = None)]
pub struct Args {
    /// Bind address (e.g. `0.0.0.0:8484`).
    #[arg(long, env = "TESELA_RELAY_BIND", default_value = "0.0.0.0:8484")]
    pub bind: SocketAddr,

    /// Path to the SQLite store. Created on first run.
    #[arg(long, env = "TESELA_RELAY_DB", default_value = "./relay.sqlite")]
    pub db: PathBuf,

    /// Admin token for `DELETE /admin/groups/{id}/register`. If unset,
    /// admin endpoints are disabled — operators who want to be able
    /// to recover from hijack squatting must set this. Pick a long
    /// random string.
    #[arg(long, env = "TESELA_RELAY_ADMIN_TOKEN")]
    pub admin_token: Option<String>,

    /// Maximum body size (bytes) accepted on PUT /ops. A single Loro doc can't
    /// be split across envelopes, so this must exceed the largest note's
    /// snapshot on the wire (the biggest real note, ai-business, is ~5 MB
    /// snapshot ≈ 7 MB encoded). Default 16 MiB — the relay only stores
    /// rate-limited ciphertext, so a generous cap is cheap. Producing clients
    /// still chunk multi-note batches (see `MAX_RELAY_PLAINTEXT_BYTES`).
    #[arg(long, env = "TESELA_RELAY_MAX_BODY", default_value_t = 16 * 1024 * 1024)]
    pub max_body: usize,

    // ── APNs silent-push (sync durability P3c). All four required to
    //    enable; any unset → push disabled, relay runs poll-only as
    //    before. Same env-var names as the Cloudflare Worker so the two
    //    relays share one ops vocabulary. An HA add-on maps its options
    //    to these env vars.
    /// APNs auth key — either the `.p8` PEM contents
    /// (`-----BEGIN PRIVATE KEY-----…`) or a filesystem path to the
    /// `.p8` file (the HA-add-on-friendly form; auto-detected).
    #[arg(long, env = "APNS_KEY_P8")]
    pub apns_key_p8: Option<String>,

    /// APNs Key ID (the `.p8`'s key id; the JWT `kid`). e.g. `C2DP446WQ9`.
    #[arg(long, env = "APNS_KEY_ID")]
    pub apns_key_id: Option<String>,

    /// Apple Developer Team ID (the JWT `iss`). e.g. `K7CBQW6MPG`.
    #[arg(long, env = "APNS_TEAM_ID")]
    pub apns_team_id: Option<String>,

    /// App bundle id (the APNs `apns-topic`). e.g. `app.tesela.ios`.
    #[arg(long, env = "APNS_BUNDLE_ID")]
    pub apns_bundle_id: Option<String>,

    /// Optional APNs host override (default `https://api.push.apple.com`).
    /// Set `https://api.sandbox.push.apple.com` for development tokens
    /// (the iOS entitlement is `aps-environment=development`).
    #[arg(long, env = "APNS_HOST")]
    pub apns_host: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    // Build the APNs sender from config (None unless all APNS_* are set +
    // the .p8 parses). A bad key fails here loudly rather than per-push.
    let apns = tesela_relay::apns::Apns::from_config(
        args.apns_key_p8.clone(),
        args.apns_key_id.clone(),
        args.apns_team_id.clone(),
        args.apns_bundle_id.clone(),
        args.apns_host.clone(),
    )
    .map(std::sync::Arc::new);
    match &apns {
        Some(_) => info!(
            "APNs silent-push ENABLED (key id {})",
            args.apns_key_id.as_deref().unwrap_or("?")
        ),
        None => info!("APNs silent-push disabled (APNS_* not fully configured)"),
    }
    let state =
        AppState::open_with_apns(&args.db, args.max_body, args.admin_token.clone(), apns).await?;
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    info!("tesela-relay listening on http://{}", args.bind);
    // `into_make_service_with_connect_info` so the per-IP rate gate
    // can extract the client `SocketAddr` via `ConnectInfo`.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
