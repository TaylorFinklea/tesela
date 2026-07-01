# Recovery-phrase + QR pairing/identity model — P0 spec

**Epic:** `tesela-ra7`. **Direction locked** 2026-06-30 (Taylor); **fork resolved:** the root secret is a **generated BIP39 recovery phrase that IS the GroupKey** (Anytype-faithful; not a user-chosen passphrase). Full rationale + grounded current-crypto map: `decisions.md` 2026-06-30.

> ⚠️ **This is convergence-critical, E2E-crypto code. TDD, and an adversarial crypto review gate before ANY of it ships to a device.** The failure mode for this kind of change is confidently-wrong crypto that compiles + passes happy-path tests. Do not rush.

## Goal (P0)

A device joins/recovers a group by entering its recovery phrase **alone** — no server URL, no reachable inviter — and existing groups adopt this with **zero re-keying**. This fixes `mp0.3` (short-code dead-on-arrival) at the root and delivers the first real recovery story (today a lost-all-devices group is unrecoverable — the random GroupKey lives only on disk).

## Current crypto (recon 2026-06-30 — mirror these, don't reinvent)

- `GroupKey` = 32-byte random, `crypto/keys.rs`; sealed content = XChaCha20-Poly1305, `crypto/aead.rs`.
- Pairing blob = postcard(group_id + **group_key bytes** + server URL + relay_url) → base64url, `crypto/pairing.rs`. Short code = 10-min in-memory map **on the inviter's server**, `routes/peer_sync.rs` (← the server-URL dependency that dead-ends fresh installs).
- `group_id` = random UUIDv4 (`group.rs`), **not** derived from the key; used as the HKDF salt for relay auth.
- Relay auth (zero-knowledge) = `auth_key = HKDF-SHA256(ikm=group_key, salt=group_id, info="tesela-relay-auth-v1")`, `crypto/relay_auth.rs`; CF Worker mirror `cloudflare-relay/src/crypto.ts`. Registration proves key-possession via HMAC.
- Ed25519 columns exist but are **dormant** (`schema.rs`, `group.rs`) — that's P2/`tesela-tp0`, not P0.

## Design

### 1. Phrase ⇄ key (new `crypto` module, e.g. `crypto/recovery.rs`)
- 32-byte GroupKey ⇔ **24-word BIP39** (256-bit entropy + checksum, standard English wordlist). The phrase is a lossless human-transcribable rendering of the existing key — **no KDF, no new key material**.
- Pure, exhaustively round-trip-tested (key→phrase→key identity; reject bad checksum / wrong word count / non-wordlist tokens). Pick a maintained BIP39 crate; mirror the existing `crypto/*` module + test style.

### 2. Relay-discoverable group handle (the one genuinely new protocol piece)
- Problem: a phrase-only device has the GroupKey but **not** the group_id (random, not derived from the key), and the relay indexes by group_id. It can't compute the auth_key (needs group_id as salt) or fetch ops.
- Fix: derive a **one-way discovery handle** `disc = HKDF-SHA256(ikm=group_key, salt="", info="tesela-group-discovery-v1")` (mirror `relay_auth.rs`'s HKDF). The relay stores a `disc → group_id` index, published by the group's registration. Recover flow: phrase → GroupKey → `disc` → `GET /discover/{disc}` → group_id → existing auth path unchanged.
- **Zero-knowledge preserved:** `disc` is a one-way PRF of the key; the relay learns a random-looking handle, never anything decryptable. **Free migration preserved:** group_id is untouched — existing groups keep their random id and just publish `disc` on next registration.
- Risks to spec out: handle **enumeration/abuse** (someone probing `/discover/*`) → the handle is 256-bit unguessable, but add rate-limiting + return only group_id (still auth-gated for ops). Registration race (two devices publish `disc`) → idempotent, same group_id.

### 3. Migration (free)
- Existing group: a "Show recovery phrase" action BIP39-encodes the **current** on-disk GroupKey. No re-key, no re-pair of existing devices. First post-upgrade registration publishes `disc`.

### 4. Surfaces
- **Desktop/web:** a "Recovery phrase" screen (reveal the 24 words behind an explicit tap + "write these down / we can't recover them" warning — mirror Anytype's copy posture).
- **New-device join:** an "Enter recovery phrase" flow. On iOS this **replaces the broken short-code path** (`PairWithShortCodeView`) — decode phrase → GroupKey → `disc` → discover → adopt, no server URL. QR keeps working as the quick same-network path (P1 can switch the QR payload to the phrase; not required for P0).

## Phasing within P0 (tier routing)
1. **Rust crypto** (Opus-spec, Senior-impl-OK under review): `crypto/recovery.rs` BIP39 ⇔ key + `disc` derivation + tests. **Verify:** `cargo test -p tesela-sync`.
2. **Relay** (Senior): `disc → group_id` index + `/discover/{disc}` on both the Rust relay AND the CF Worker (`cloudflare-relay`), conformance-parity. **Verify:** relay conformance suite.
3. **FFI + join flow** (Senior): FFI to decode phrase → adopt via discovery; iOS "Enter recovery phrase" replacing the short-code path; "Show recovery phrase" screen. **Verify:** `xcodebuild test` + a sim/device recover-from-phrase round trip.

## Open questions (resolve before/within impl)
- Word count: 24 (256-bit, matches the 32-byte key exactly) vs 12 (128-bit — would truncate). **Default 24** (lossless).
- Do we keep the current raw-GroupKey QR blob for P0 (quick path) and only add the phrase path? **Yes** — additive, lowest risk.
- Optional local "vault lock" (a memorable password that only locally encrypts the stored phrase) — explicitly **deferred** (additive, post-P0).
- P2 (`tesela-tp0`): Ed25519 device identity + signed relay registration — separate epic phase; not P0.

## Non-goals (P0)
- User-chosen passphrase / Argon2 (fork rejected). Ed25519 identity (P2). Multi-user/ACL (Savanne).
