//! Sync op schema migration. Translators map older `OpPayload` shapes
//! forward so newer devices can apply older devices' ops.
//!
//! Phase 1 has only `SYNC_SCHEMA_VERSION = 1`, so no translators exist yet.
//! The `TranslatorRegistry` shape is in place so v1-to-v2 lands as a
//! mechanical addition when the first op-shape change happens.

use crate::error::{SyncError, SyncResult};
use crate::oplog::op::OpPayload;
use std::collections::BTreeMap;

/// Pure translator from one op schema version to the next.
pub trait OpTranslator: Send + Sync {
    /// Schema version the translator accepts as input.
    fn source_version(&self) -> u32;
    /// Schema version the translator produces.
    fn to_version(&self) -> u32;
    /// Translate one payload.
    fn translate(&self, payload: OpPayload) -> SyncResult<OpPayload>;
}

/// Registry of translators. Looks up shortest chain from version A to B.
#[derive(Default)]
pub struct TranslatorRegistry {
    by_from: BTreeMap<(u32, u32), Box<dyn OpTranslator>>,
}

impl TranslatorRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a translator. Later registrations replace earlier ones
    /// for the same `(from, to)` pair.
    pub fn register(&mut self, t: Box<dyn OpTranslator>) {
        self.by_from.insert((t.source_version(), t.to_version()), t);
    }

    /// Find the chain of translators that takes `from` to `to`. Phase 1
    /// only supports straight-line `+1` chains, which matches the
    /// expected evolution path. Returns `None` if no chain exists.
    pub fn chain(&self, from: u32, to: u32) -> Option<Vec<&dyn OpTranslator>> {
        if from == to {
            return Some(vec![]);
        }
        if from > to {
            return None;
        }
        let mut chain = Vec::new();
        let mut cur = from;
        while cur < to {
            let next = cur + 1;
            let t = self.by_from.get(&(cur, next))?;
            chain.push(t.as_ref());
            cur = next;
        }
        Some(chain)
    }

    /// Apply the chain end-to-end.
    pub fn translate(&self, from: u32, to: u32, mut payload: OpPayload) -> SyncResult<OpPayload> {
        let chain = self
            .chain(from, to)
            .ok_or(SyncError::NoTranslatorChain { from, to })?;
        for t in chain {
            payload = t.translate(payload)?;
        }
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bump(u32, u32);
    impl OpTranslator for Bump {
        fn source_version(&self) -> u32 {
            self.0
        }
        fn to_version(&self) -> u32 {
            self.1
        }
        fn translate(&self, payload: OpPayload) -> SyncResult<OpPayload> {
            Ok(payload)
        }
    }

    #[test]
    fn chain_v1_to_v3_via_two_translators() {
        let mut reg = TranslatorRegistry::new();
        reg.register(Box::new(Bump(1, 2)));
        reg.register(Box::new(Bump(2, 3)));
        let chain = reg.chain(1, 3).expect("chain exists");
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn no_chain_when_translator_missing() {
        let mut reg = TranslatorRegistry::new();
        reg.register(Box::new(Bump(1, 2)));
        // 2 to 3 missing
        assert!(reg.chain(1, 3).is_none());
    }

    #[test]
    fn empty_chain_when_versions_equal() {
        let reg = TranslatorRegistry::new();
        let chain = reg.chain(1, 1).expect("equal returns empty chain");
        assert!(chain.is_empty());
    }
}
