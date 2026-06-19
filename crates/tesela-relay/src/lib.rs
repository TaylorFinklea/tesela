//! `tesela-relay` library surface — the same router + state plumbing
//! the binary uses, exposed so integration tests can spawn an
//! in-process server on a random port and hit it with reqwest. Also
//! the entry point for the eventual `tesela-relay-conformance` crate
//! when the Cloudflare Worker port arrives (stage 7) and we want one
//! test suite checking both deployments.

use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, post, put};
use axum::Router;

pub mod apns;
pub mod handlers;
pub mod state;
pub mod store;

pub use state::AppState;

/// Tesela-sync's `tesela_sync` is a transitive dep through the
/// integration tests; this re-export keeps a single place to point
/// other callers (eventually `tesela-server` + `tesela-sync` clients)
/// at the canonical relay-auth primitives. No-op for the binary.
pub use tesela_sync::crypto::relay_auth;

/// Build the relay's HTTP router from a fully-initialised
/// `AppState`. Used by both `main.rs` (production bind) and the
/// integration tests (random-port spawn).
///
/// Endpoint layout:
///
/// - `GET /`                                — health
/// - `POST /groups/{id}/register`           — open (registration bootstrap)
/// - `GET  /groups/{id}/registration`       — open (joiner verifies)
/// - `PUT  /groups/{id}/ops`                — MAC-gated
/// - `GET  /groups/{id}/ops`                — MAC-gated
/// - `POST /groups/{id}/ack`                — MAC-gated
/// - `POST /groups/{id}/devices`            — MAC-gated (APNs token registry, P3b)
/// - `PUT  /groups/{id}/snapshot`           — MAC-gated (snapshot deposit + compaction)
/// - `GET  /groups/{id}/snapshots`          — MAC-gated (bootstrap source)
/// - `DELETE /admin/groups/{id}/register`   — admin-token-gated (handler checks)
pub fn router(state: AppState) -> Router {
    // Routes that the MAC middleware gates. Separate sub-router so we
    // can layer the middleware only over endpoints that require it —
    // /register can't MAC-verify (no auth_key stored yet).
    let mac_gated = Router::new()
        .route(
            "/groups/{group_id}/ops",
            put(handlers::put_op).get(handlers::get_ops),
        )
        .route("/groups/{group_id}/ack", post(handlers::post_ack))
        .route(
            "/groups/{group_id}/devices",
            post(handlers::handle_register_device),
        )
        .route("/groups/{group_id}/snapshot", put(handlers::put_snapshot))
        .route("/groups/{group_id}/snapshots", get(handlers::get_snapshots))
        .layer(from_fn_with_state(state.clone(), handlers::mac_gate));

    Router::new()
        .route("/", get(handlers::health))
        .route("/groups/{group_id}/register", post(handlers::register))
        .route(
            "/groups/{group_id}/registration",
            get(handlers::get_registration),
        )
        .route(
            "/admin/groups/{group_id}/register",
            delete(handlers::admin_delete_registration),
        )
        .merge(mac_gated)
        // Per-IP rate limit runs first so even pre-auth scan traffic
        // gets throttled. Stacked over everything so /register,
        // /registration, /ops, /ack, and /admin all count toward the
        // window cap.
        .layer(from_fn_with_state(state.clone(), handlers::rate_gate))
        .with_state(state)
}
