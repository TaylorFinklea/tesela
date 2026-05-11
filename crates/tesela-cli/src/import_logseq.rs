//! Thin re-export — the actual Logseq importer lives in `tesela-core`
//! so both the CLI and the server can call its `build_plan` /
//! `apply_plan` functions directly without shelling out and JSON-piping.

pub use tesela_core::import_logseq::*;
