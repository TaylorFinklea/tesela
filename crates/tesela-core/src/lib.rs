pub mod config;
pub mod daily;
pub mod db;
pub mod error;
pub mod export;
pub mod indexer;
pub mod link;
pub mod note;
pub mod storage;
pub mod tag;
pub mod traits;

// Re-export key types at crate root
pub use config::Config;
pub use db::SqliteIndex;
pub use error::{Result, TeselaError};
pub use indexer::{Indexer, IndexerHandle, NoteEvent};
pub use link::{Link, LinkType};
pub use note::{Note, NoteId, NoteMetadata, SearchHit};
pub use tag::Tag;
