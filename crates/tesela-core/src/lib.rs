pub mod config;
pub mod daily;
pub mod error;
pub mod export;
pub mod link;
pub mod note;
pub mod storage;
pub mod tag;
pub mod traits;

// Re-export key types at crate root
pub use config::Config;
pub use error::{Result, TeselaError};
pub use link::{Link, LinkType};
pub use note::{Note, NoteId, NoteMetadata, SearchHit};
pub use tag::Tag;
