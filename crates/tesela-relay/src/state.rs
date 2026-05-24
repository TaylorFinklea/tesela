//! Shared application state — clone-cheap, immutable after startup.

use std::sync::Arc;

use anyhow::Result;

use crate::store::Store;

/// Cloneable handle holding everything the request handlers need.
/// Wrapped in `Arc` so handlers can share without per-request locking
/// on the inner state.
#[derive(Clone)]
pub struct AppState {
    // Read in stages 3a-3d once handlers grow past the health probe;
    // committing the field up front so the AppState shape is stable
    // before handlers depend on it.
    #[allow(dead_code)]
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    // Wired by stage 3a (storage), 3c (max_body cap on PUT /ops), and
    // 3d (admin recovery endpoint). Suppressed unused-warnings here
    // because the skeleton commits these fields up front so the State
    // shape stabilises before handlers depend on it.
    #[allow(dead_code)]
    pub(crate) store: Store,
    #[allow(dead_code)]
    pub(crate) max_body: usize,
    /// Set iff `--admin-token` was passed. `None` means admin
    /// endpoints are disabled and respond `404`.
    #[allow(dead_code)]
    pub(crate) admin_token: Option<String>,
}

impl AppState {
    pub async fn open(args: &crate::Args) -> Result<Self> {
        let store = Store::open(&args.db).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                max_body: args.max_body,
                admin_token: args.admin_token.clone(),
            }),
        })
    }
}
