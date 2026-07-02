//! The append-only oplog.

pub mod op;
pub mod retention;

pub use op::{ContentHash, EncodedOp, OpKind, OpPayload};
