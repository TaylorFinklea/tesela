//! GET /commands — serves the built-in command manifest (tesela-cmdd.2).
//!
//! The manifest is data, not behavior: id/verb/label/glyph/category/default
//! shortcut+chord/surfaces/keywords/args-shape, NO run closures (those stay
//! native to each client). It is embedded at compile time from the ONE
//! checked-in source, `web/src/lib/command-manifest.json` — produced by
//! `web/scripts/generate-command-manifest.mjs` from the REAL, live web
//! `commandRegistry` (`commandRegistry.manifest()`), never hand-typed here.
//! This mirrors the conformance-fixture precedent (one checked-in JSON, read
//! directly by both Rust and TS — see
//! `crates/tesela-core/tests/fixtures/property-override-conformance.json`),
//! adapted for a manifest that must also be served at runtime, not just
//! consumed by tests.

use std::sync::LazyLock;

use axum::Json;
use serde::{Deserialize, Serialize};

const MANIFEST_JSON: &str =
    include_str!("../../../../web/src/lib/command-manifest.json");

/// One command's metadata. Field names are snake_case to match the checked-in
/// JSON verbatim (no camelCase translation layer at the boundary — same
/// convention as `Note`/`PropertyDef`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandManifestEntry {
    pub id: String,
    pub verb: Option<String>,
    pub label: String,
    pub glyph: String,
    pub category: String,
    pub shortcut: Option<String>,
    pub chord: Option<Vec<String>>,
    pub surfaces: Vec<String>,
    pub keywords: Vec<String>,
    pub takes_arg: bool,
    pub arg_prompt: Option<String>,
}

static MANIFEST: LazyLock<Vec<CommandManifestEntry>> = LazyLock::new(|| {
    serde_json::from_str(MANIFEST_JSON)
        .expect("web/src/lib/command-manifest.json must parse as Vec<CommandManifestEntry>")
});

/// GET /commands — every registered command's metadata. Unauthenticated,
/// like `/health`/`/info`: it's static built-in data, not mosaic content.
pub async fn list_commands() -> Json<Vec<CommandManifestEntry>> {
    Json(MANIFEST.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // No AppState/router needed — `list_commands` takes no extractors, so
    // this exercises the real production handler (compile-time-embedded JSON
    // + serde parse + the fn axum actually routes to) without spawning a
    // server, avoiding the known bind-timeout/port-TOCTOU flake under
    // parallel test runs (tesela-6c6).

    #[tokio::test]
    async fn list_commands_returns_every_registered_command_non_empty() {
        let Json(commands) = list_commands().await;
        assert!(!commands.is_empty(), "manifest must not be empty");
    }

    #[tokio::test]
    async fn list_commands_ids_are_unique() {
        let Json(commands) = list_commands().await;
        let ids: HashSet<&str> = commands.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), commands.len(), "duplicate ids in the manifest");
    }

    #[tokio::test]
    async fn list_commands_entries_have_no_closures_only_data() {
        // Compile-time proof: CommandManifestEntry has no `run`/`when` fields
        // at all (unlike the web `Command` type) — this test asserts the
        // required data fields are populated, since a manifest entry with an
        // empty id/label/category would silently defeat every consumer.
        let Json(commands) = list_commands().await;
        for c in &commands {
            assert!(!c.id.is_empty(), "command missing id");
            assert!(!c.label.is_empty(), "{}: missing label", c.id);
            assert!(!c.category.is_empty(), "{}: missing category", c.id);
            assert!(!c.surfaces.is_empty(), "{}: not visible on any surface", c.id);
        }
    }
}
