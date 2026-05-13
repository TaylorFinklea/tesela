//! Cursors: the engine's "where am I in time" pointers.

use crate::hlc::HlcTimestamp;
use serde::{Deserialize, Serialize};

/// A cursor over locally-produced ops.
///
/// `Earliest` means "from the beginning of the oplog." Otherwise the
/// cursor names the HLC of the most-recent op the holder has seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalCursor {
    /// Beginning of the oplog (we have produced nothing, or peer requests a full replay).
    Earliest,
    /// Cursor at the HLC of the most-recent op produced locally.
    At(HlcTimestamp),
}

/// A cursor over ops received from a specific peer.
///
/// Deliberately distinct from [`LocalCursor`] so the type system catches
/// "send me everything after my local cursor" misuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerCursor {
    /// Beginning of the peer's oplog (we want a full replay).
    Earliest,
    /// We have seen everything from this peer up to (and including) this HLC.
    At(HlcTimestamp),
}

impl PeerCursor {
    /// Whether this cursor strictly precedes the given HLC. Used by the
    /// engine when computing "send me ops after C."
    pub fn strictly_before(&self, ts: HlcTimestamp) -> bool {
        match self {
            PeerCursor::Earliest => true,
            PeerCursor::At(c) => *c < ts,
        }
    }
}

impl LocalCursor {
    /// As above, for local cursors.
    pub fn strictly_before(&self, ts: HlcTimestamp) -> bool {
        match self {
            LocalCursor::Earliest => true,
            LocalCursor::At(c) => *c < ts,
        }
    }
}
