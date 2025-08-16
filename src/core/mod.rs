//! Core functionality for Tesela
//!
//! This module contains the fundamental components that power Tesela's
//! note-taking capabilities, including storage, database, configuration,
//! and error handling.

pub mod config;
pub mod database;
pub mod error;
pub mod indexer;
pub mod search;
pub mod storage;

// Re-export commonly used types
pub use config::{Config, ConfigBuilder};
pub use database::{Database, DatabaseConfig};
pub use error::{Result, TeselaError};
pub use indexer::{IndexEvent, Indexer, IndexerConfig};
pub use search::{SearchConfig, SearchEngine, SearchResult};
pub use storage::{Note, Storage, StorageConfig};

// Future modules (to be implemented):
// pub mod api;      // Core API layer for async operations
// pub mod cache;    // LRU cache implementation
