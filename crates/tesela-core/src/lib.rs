pub mod block;
pub mod config;
pub mod daily;
pub mod db;
pub mod error;
pub mod export;
pub mod import_logseq;
pub mod indexer;
pub mod link;
pub mod note;
pub mod note_tree;
pub mod query;
pub mod recurrence;
pub mod regex_cache;
pub mod storage;
pub mod system_widgets;
pub mod tag;
pub mod traits;
pub mod types;

// Re-export key types at crate root
pub use config::Config;
pub use db::SqliteIndex;
pub use error::{Result, TeselaError};
pub use indexer::{Indexer, IndexerHandle, NoteEvent};
pub use link::{GraphEdge, Link, LinkType};
pub use note::{Note, NoteId, NoteMetadata, SearchHit};
pub use tag::Tag;
