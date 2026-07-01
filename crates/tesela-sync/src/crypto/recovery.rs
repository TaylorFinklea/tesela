//! Recovery-phrase encoding for the group key (`tesela-ra7` P0 step 1).
//!
//! See `.docs/ai/phases/2026-06-30-recovery-phrase-spec.md`. The recovery
//! phrase IS the [`GroupKey`] -- a lossless, human-transcribable BIP39
//! rendering of the existing 32-byte key. There is no KDF here and no new
//! key material: `phrase_to_key(key_to_phrase(k)) == k` for every `k`.
//!
//! This module also derives the relay **discovery handle**: a one-way,
//! `group_id`-independent PRF of the group key that lets a phrase-only
//! device (which has the key but not the random `group_id`) ask the relay
//! "which group does this key belong to?" without revealing the key
//! itself. See [`derive_discovery_handle`].

use bip39::{Language, Mnemonic};
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;

use crate::crypto::keys::GroupKey;

/// Domain-separator for the discovery-handle derivation. MUST stay
/// distinct from `crate::crypto::relay_auth::RELAY_AUTH_INFO_V1` -- see
/// `discovery_handle_differs_from_relay_auth_key` below. Treat as
/// immutable post-v1, mirroring `relay_auth`'s versioning convention.
pub const GROUP_DISCOVERY_INFO_V1: &[u8] = b"tesela-group-discovery-v1";

/// Word count for a Tesela recovery phrase: 24 English BIP39 words = 256
/// bits of entropy, matching the `GroupKey`'s 32 bytes exactly and
/// losslessly. (12/15/18/21-word phrases are valid BIP39 but would only
/// capture 128-224 bits and truncate the key, so they're rejected here
/// even though the underlying crate accepts them in general.)
pub const RECOVERY_PHRASE_WORD_COUNT: usize = 24;

/// Errors from phrase <-> key conversion. Every variant carries only
/// counts or positions -- never phrase words, entropy, or key bytes --
/// mirroring `GroupKey`'s no-leak `Debug` posture.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    /// Phrase did not have exactly [`RECOVERY_PHRASE_WORD_COUNT`] words.
    #[error("recovery phrase must be {expected} words, got {actual}")]
    WordCount {
        /// Required word count.
        expected: usize,
        /// Word count actually found.
        actual: usize,
    },
    /// The word at this (0-based) position isn't in the BIP39 English
    /// wordlist.
    #[error("word at position {index} is not a valid recovery-phrase word")]
    UnknownWord {
        /// Index of the offending word.
        index: usize,
    },
    /// Checksum bits didn't match -- the phrase was mistyped, reordered,
    /// or otherwise corrupted.
    #[error("recovery phrase checksum is invalid")]
    InvalidChecksum,
    /// Any other rejection from the underlying BIP39 parser that doesn't
    /// have a dedicated variant above.
    #[error("recovery phrase is invalid")]
    Invalid,
}

impl RecoveryError {
    fn from_bip39(e: bip39::Error) -> Self {
        match e {
            bip39::Error::BadWordCount(actual) => RecoveryError::WordCount {
                expected: RECOVERY_PHRASE_WORD_COUNT,
                actual,
            },
            bip39::Error::UnknownWord(index) => RecoveryError::UnknownWord { index },
            bip39::Error::InvalidChecksum => RecoveryError::InvalidChecksum,
            _ => RecoveryError::Invalid,
        }
    }
}

/// Encode a `GroupKey` as its 24-word BIP39 English recovery phrase.
/// Pure and lossless -- `phrase_to_key` is the exact inverse for every
/// possible 32-byte key (see `phrase_round_trips_for_random_keys`).
pub fn key_to_phrase(key: &GroupKey) -> String {
    let mnemonic = Mnemonic::from_entropy(key.as_bytes())
        .expect("a 32-byte GroupKey is always valid BIP39 entropy (256 bits, English wordlist)");
    mnemonic.to_string()
}

/// Parse and VALIDATE a recovery phrase back into a `GroupKey`. Trims
/// surrounding whitespace and lowercases before matching against the
/// wordlist (BIP39 English words are ASCII-lowercase; any run of
/// whitespace between words is treated as a single separator). Rejects
/// -- never silently accepts -- a wrong word count, a token that isn't
/// in the wordlist, or a bad checksum.
pub fn phrase_to_key(phrase: &str) -> Result<GroupKey, RecoveryError> {
    let normalized = phrase.trim().to_lowercase();
    let word_count = normalized.split_whitespace().count();
    if word_count != RECOVERY_PHRASE_WORD_COUNT {
        return Err(RecoveryError::WordCount {
            expected: RECOVERY_PHRASE_WORD_COUNT,
            actual: word_count,
        });
    }

    let mnemonic = Mnemonic::parse_in_normalized(Language::English, &normalized)
        .map_err(RecoveryError::from_bip39)?;

    let entropy = mnemonic.to_entropy();
    if entropy.len() != 32 {
        // Unreachable given the word-count gate above (24 English BIP39
        // words <=> exactly 32 bytes of entropy); kept as a
        // defense-in-depth check rather than an `unreachable!()` since
        // this is security-sensitive code.
        return Err(RecoveryError::WordCount {
            expected: RECOVERY_PHRASE_WORD_COUNT,
            actual: word_count,
        });
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&entropy);
    Ok(GroupKey::from_bytes(bytes))
}

/// Derive the one-way, `group_id`-independent discovery handle for a
/// group key: `HKDF-SHA256(salt=None, ikm=group_key,
/// info=GROUP_DISCOVERY_INFO_V1)`. Mirrors
/// `relay_auth::derive_relay_auth_key`'s construction, but with `salt =
/// None` -- a phrase-only device recovering has the `GroupKey` but not
/// yet the `group_id` (the random UUID the relay actually indexes by).
/// That's the whole point of this handle: the relay maintains a
/// `disc -> group_id` index, published at registration time, so a
/// phrase-only device can look up its `group_id` before it can compute
/// anything that needs `group_id` as input (like the relay auth key).
///
/// Domain-separated from `RELAY_AUTH_INFO_V1` by construction (distinct
/// `info` string), so this handle can never equal the relay auth key
/// for the same key -- see
/// `discovery_handle_differs_from_relay_auth_key_for_same_key`.
pub fn derive_discovery_handle(key: &GroupKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, key.as_bytes());
    let mut out = [0u8; 32];
    // HKDF-Expand can produce up to 255 * Sha256::OutputSize bytes; 32 is
    // trivially safe so the `expect` here can never fire.
    hk.expand(GROUP_DISCOVERY_INFO_V1, &mut out)
        .expect("32-byte HKDF output is well below the max");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::relay_auth::derive_relay_auth_key;
    use crate::group::GroupId;
    use rand::RngCore;

    fn random_key() -> GroupKey {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        GroupKey::from_bytes(bytes)
    }

    // --- round-trip ---------------------------------------------------

    #[test]
    fn phrase_round_trips_for_random_keys() {
        for _ in 0..256 {
            let key = random_key();
            let phrase = key_to_phrase(&key);
            let recovered = phrase_to_key(&phrase).expect("valid phrase must parse");
            assert_eq!(recovered.as_bytes(), key.as_bytes());
        }
    }

    #[test]
    fn phrase_round_trips_for_edge_case_keys() {
        for bytes in [[0x00u8; 32], [0xffu8; 32], {
            let mut b = [0u8; 32];
            for (i, byte) in b.iter_mut().enumerate() {
                *byte = i as u8;
            }
            b
        }] {
            let key = GroupKey::from_bytes(bytes);
            let phrase = key_to_phrase(&key);
            let recovered = phrase_to_key(&phrase).expect("valid phrase must parse");
            assert_eq!(recovered.as_bytes(), key.as_bytes());
        }
    }

    #[test]
    fn key_to_phrase_produces_24_words() {
        let key = random_key();
        let phrase = key_to_phrase(&key);
        assert_eq!(phrase.split_whitespace().count(), RECOVERY_PHRASE_WORD_COUNT);
    }

    // --- determinism ----------------------------------------------------

    #[test]
    fn key_to_phrase_is_deterministic() {
        let key = GroupKey::from_bytes([0x42; 32]);
        assert_eq!(key_to_phrase(&key), key_to_phrase(&key));
    }

    #[test]
    fn phrase_to_key_is_deterministic() {
        let key = GroupKey::from_bytes([0x42; 32]);
        let phrase = key_to_phrase(&key);
        let a = phrase_to_key(&phrase).unwrap();
        let b = phrase_to_key(&phrase).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn phrase_to_key_tolerates_whitespace_and_case() {
        let key = GroupKey::from_bytes([0x13; 32]);
        let phrase = key_to_phrase(&key);
        let mangled = format!(
            "  \n{}\t\n ",
            phrase
                .split_whitespace()
                .map(|w| w.to_uppercase())
                .collect::<Vec<_>>()
                .join("   ")
        );
        let recovered = phrase_to_key(&mangled).expect("uppercased/whitespace-mangled phrase should still parse");
        assert_eq!(recovered.as_bytes(), key.as_bytes());
    }

    // --- rejection --------------------------------------------------------

    #[test]
    fn phrase_to_key_rejects_empty_string() {
        let err = phrase_to_key("").unwrap_err();
        assert_eq!(
            err,
            RecoveryError::WordCount {
                expected: RECOVERY_PHRASE_WORD_COUNT,
                actual: 0,
            }
        );
    }

    #[test]
    fn phrase_to_key_rejects_12_words() {
        // A genuinely valid 12-word BIP39 mnemonic (128 bits, 16 bytes of
        // entropy) -- valid BIP39 in its own right, but not a valid
        // Tesela recovery phrase (would only capture half the key).
        let twelve = Mnemonic::from_entropy(&[0u8; 16]).unwrap().to_string();
        // Sanity: this is the well-known canonical 12-word
        // all-zero-entropy mnemonic.
        assert_eq!(twelve, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        let err = phrase_to_key(&twelve).unwrap_err();
        assert_eq!(
            err,
            RecoveryError::WordCount {
                expected: RECOVERY_PHRASE_WORD_COUNT,
                actual: 12,
            }
        );
    }

    #[test]
    fn phrase_to_key_rejects_23_words() {
        let key = random_key();
        let phrase = key_to_phrase(&key);
        let words: Vec<&str> = phrase.split_whitespace().collect();
        let short = words[..23].join(" ");
        let err = phrase_to_key(&short).unwrap_err();
        assert_eq!(
            err,
            RecoveryError::WordCount {
                expected: RECOVERY_PHRASE_WORD_COUNT,
                actual: 23,
            }
        );
    }

    #[test]
    fn phrase_to_key_rejects_25_words() {
        let key = random_key();
        let phrase = key_to_phrase(&key);
        let mut words: Vec<&str> = phrase.split_whitespace().collect();
        words.push("abandon");
        let long = words.join(" ");
        let err = phrase_to_key(&long).unwrap_err();
        assert_eq!(
            err,
            RecoveryError::WordCount {
                expected: RECOVERY_PHRASE_WORD_COUNT,
                actual: 25,
            }
        );
    }

    #[test]
    fn phrase_to_key_rejects_non_wordlist_token() {
        let key = random_key();
        let phrase = key_to_phrase(&key);
        let mut words: Vec<&str> = phrase.split_whitespace().collect();
        words[5] = "notarealbip39word";
        let bogus = words.join(" ");
        let err = phrase_to_key(&bogus).unwrap_err();
        assert_eq!(err, RecoveryError::UnknownWord { index: 5 });
    }

    #[test]
    fn phrase_to_key_rejects_bad_checksum() {
        // Deterministic fixture: the all-zero-key phrase's first word
        // flipped from "abandon" to another valid wordlist word ("zoo")
        // preserves the word count and every word is individually valid,
        // but the trailing checksum word no longer matches --
        // `InvalidChecksum`, not `UnknownWord`.
        let key = GroupKey::from_bytes([0u8; 32]);
        let phrase = key_to_phrase(&key);
        let mut words: Vec<&str> = phrase.split_whitespace().collect();
        assert_eq!(words[0], "abandon");
        words[0] = "zoo";
        let mutated = words.join(" ");
        let err = phrase_to_key(&mutated).unwrap_err();
        assert_eq!(err, RecoveryError::InvalidChecksum);
    }

    #[test]
    fn recovery_error_display_never_contains_words() {
        // Spot-check that error Display strings only ever carry counts/
        // indices, never phrase content -- even for a fixture whose
        // words might otherwise leak into a naive error message.
        let key = GroupKey::from_bytes([0x77; 32]);
        let phrase = key_to_phrase(&key);
        let words: Vec<&str> = phrase.split_whitespace().collect();
        for w in &words {
            let err = RecoveryError::UnknownWord { index: 3 };
            assert!(!err.to_string().contains(w));
            let err = RecoveryError::WordCount {
                expected: 24,
                actual: 12,
            };
            assert!(!err.to_string().contains(w));
        }
    }

    // --- discovery handle -------------------------------------------------

    #[test]
    fn discovery_handle_is_deterministic() {
        let key = GroupKey::from_bytes([0x9a; 32]);
        assert_eq!(derive_discovery_handle(&key), derive_discovery_handle(&key));
    }

    #[test]
    fn discovery_handle_is_unique_per_key() {
        let a = random_key();
        let b = random_key();
        assert_ne!(derive_discovery_handle(&a), derive_discovery_handle(&b));
    }

    /// The discovery handle MUST be a distinct domain from the relay
    /// auth key for the *same* group key -- otherwise a value computed
    /// for one purpose could be replayed as the other. If this ever
    /// flips (e.g. someone "simplifies" the info string), the relay
    /// could be tricked into treating a discovery probe as an
    /// authenticated request or vice versa.
    #[test]
    fn discovery_handle_differs_from_relay_auth_key_for_same_key() {
        let key = random_key();
        let disc = derive_discovery_handle(&key);
        for group_id in [
            GroupId::new_random(),
            GroupId::from_bytes([0u8; 16]),
            GroupId::from_bytes([0xff; 16]),
        ] {
            let auth = derive_relay_auth_key(&key, &group_id);
            assert_ne!(disc, auth);
        }
    }

    #[test]
    fn discovery_info_const_is_distinct_from_relay_auth_info() {
        assert_ne!(
            GROUP_DISCOVERY_INFO_V1,
            crate::crypto::relay_auth::RELAY_AUTH_INFO_V1
        );
    }
}
