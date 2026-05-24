//! `tesela-relay` library surface — the same router + state plumbing
//! the binary uses, exposed so integration tests can spawn an
//! in-process server on a random port and hit it with reqwest. Also
//! the entry point for the eventual `tesela-relay-conformance` crate
//! when the Cloudflare Worker port arrives (stage 7) and we want one
//! test suite checking both deployments.

use axum::{routing::get, Router};

pub mod handlers;
pub mod state;
pub mod store;

pub use state::AppState;

/// Build the relay's HTTP router from a fully-initialised
/// `AppState`. Used by both `main.rs` (production bind) and the
/// integration tests (random-port spawn).
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::health))
        // Handlers are stubbed for stage 2a; stages 3a-3d fill them in.
        .with_state(state)
}
