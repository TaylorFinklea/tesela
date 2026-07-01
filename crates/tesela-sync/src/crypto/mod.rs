//! Crypto module. Phase 2 placeholder.
//!
//! The plan reserves this for AEAD primitives, group-key derivation, and
//! the pairing flow. Phase 1 leaves it empty so the module structure is
//! complete from day one and Phase 1 transports can carry cleartext
//! envelopes without any crypto on the call path.

pub mod aead;
pub mod keys;
pub mod pairing;
pub mod recovery;
pub mod relay_auth;
