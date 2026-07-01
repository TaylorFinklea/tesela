# GroupKey rotation as a first-class operation — design spec

**Bead:** `tesela-tp0.1`. **Type:** design spec only (no code). **Status:** draft for Lead review.
**Epic:** `tesela-tp0` (the minimum-key/pairing hardening epic that follows `tesela-ra7`'s recovery-phrase work).
**Depends on (all landed):** `tesela-ra7` P0 — BIP39 phrase⇔key, one-way discovery handle, `/discover/{disc}` on both relays, `recover_pairing_from_phrase` FFI.

> ⚠️ **Convergence-critical E2E-crypto.** This is the primitive that retires a compromised key. When it ships it MUST be TDD'd and pass an adversarial crypto review gate (same bar as the recovery-phrase work — see `2026-06-30-recovery-phrase-spec.md`). This document is design only; it prescribes no code blocks — implementers read the named modules and mirror them.

---

## 1. Problem

Today the GroupKey is minted once at genesis (`GroupKey::random()`, `crypto/keys.rs`) and never changes. `tesela-ra7` made that key **human-transcribable** (the 24-word BIP39 phrase IS the key) and **recoverable** (phrase → `disc` → `/discover` → `group_id` → adopt). It gave the group a durable identity but **no way to retire it.**

There is no operation for: "a device was lost/stolen," "the recovery phrase leaked," or "I want to exclude a device I previously trusted." Because the phrase IS the key and the key never rotates, once a phrase escapes the group there is **no remediation** — every holder of it can read all group content forever, and can sync into the group via the relay. Rotation is the missing remediation.

This spec designs GroupKey rotation as a first-class, crash-safe, zero-knowledge-preserving operation, and defines the substrate `tesela-tp0.3` (multi-user / per-member keys) will build on.

## 2. Grounded current primitives (mirror these — do not reinvent)

- **GroupKey** = 32 random bytes, `crypto/keys.rs`; stored raw at `<mosaic>/.tesela/group_key.bin` behind the `GroupKeyStore` seam. **GroupId** = random UUIDv4, `group.rs`; stored at `<mosaic>/.tesela/group_id.hex`. The two-file split exists *specifically so the key can rotate without re-issuing the id* (`keys.rs` doc comment) — but nothing consumes that capability yet.
- **`adopt(mosaic_root, &GroupIdentity)`** (`crypto/keys.rs`) overwrites both files in place, idempotently. This is the local identity-swap primitive rotation reuses.
- **`auth_key = HKDF-SHA256(ikm=group_key, salt=group_id, info="tesela-relay-auth-v1")`** (`crypto/relay_auth.rs`). The relay verifies per-request MACs against it. **The auth key depends on BOTH the key AND the group_id** — rotating either changes it.
- **`disc = HKDF-SHA256(ikm=group_key, salt=None, info="tesela-group-discovery-v1")`** (`crypto/recovery.rs`). **Depends on the key ALONE** — a new key yields a new `disc` automatically, independent of group_id.
- **Content** sealed XChaCha20-Poly1305 under the group key, `crypto/aead.rs` (envelope ops + per-note snapshots). The relay never holds the key — **zero-knowledge**.
- **Relay registration** (`tesela-relay/src/handlers.rs::register`, CF mirror `cloudflare-relay/src/handlers.ts`): `POST /groups/{group_id}/register` with `{auth_key_b64, registered_at, intent_b64, disc_b64?}`. Idempotent on byte-identical re-register; **returns 409 Conflict on a different auth_key for the same group_id — it does NOT overwrite.** When `disc_b64` is present it upserts the `disc → group_id` index.
- **Discovery** `GET /discover/{disc}` → `{group_id}` (unauthenticated; the bootstrap before a device can compute anything needing group_id).
- **Encrypted replica** — since the 2026-06-03 spine the relay KEEPS the full encrypted op log + per-note snapshots (`handlers.rs` ack note); a fresh/recovered device bootstraps from `GET /snapshots` then `GET /ops?since=`.
- **Teardown today** = admin-only: `DELETE /admin/groups/{group_id}/register`, bearer-gated on `--admin-token`, 404 when the relay started without one. The `relay_discovery_index`, `relay_ops`, `relay_snapshots`, `relay_device_tokens`, `relay_device_seen`, `relay_group_meta` tables all carry `FOREIGN KEY (group_id) REFERENCES relay_registrations(group_id) ON DELETE CASCADE` (`store.rs`), so **one registration delete cascade-wipes the entire group's footprint — including the old `disc` index row and all old-key ciphertext.**

## 3. Core decision — rotation mints a NEW `group_id` (not just a new key)

Rotation mints a fresh `(group_key, group_id)` pair — `K_new = GroupKey::random()`, `G_new = GroupId::new_random()` — registered on the relay as a brand-new group, then the old group is torn down. **The rotation does not reuse `G_old`.** Reasoning, from zero-knowledge + the `disc` derivation:

- **Crash-safety demands a distinct namespace.** `register_group` returns **409 Conflict** on a new auth_key for an existing group_id — it will not overwrite. Reusing `G_old` therefore forces *teardown-old-BEFORE-register-new* (delete the old registration so the new auth_key can be inserted). That ordering has a fatal window: after the cascade delete and before the new register, the group has **no registration and no encrypted replica on the relay** — offline devices can't sync and a crash strands the group. A **new `G_new` is always `Inserted`, never `Conflict`**, so we can register-new-FIRST and teardown-old-LAST — at every crash point either the old group is fully intact or both exist (§5).
- **`disc` already rotates for free.** `disc = HKDF(key)` alone, so `K_new` gives `D_new` automatically. Re-adoption is *exactly the recover-from-phrase path with the new phrase* (`D_new` → `/discover` → `G_new` → adopt), no new client code path. Reusing `G_old` gains nothing here — `disc` changes regardless.
- **Strongest zero-knowledge posture.** To the relay, `(G_new, auth_new, D_new)` is **indistinguishable from a brand-new group** — a fresh random id, a random-looking handle, a random-looking auth key, ciphertext it can't read. The relay cannot link `G_old → G_new`; it can only observe that one group went quiet and another appeared. Reusing `G_old` would let the relay observe "same group, key changed" (a linkage + a signal that a rotation/revocation happened). Minting `G_new` leaks strictly less.
- **Clean replica.** `G_new` starts with only `K_new`-sealed snapshots; there is never stale `K_old` ciphertext sitting under the live group_id.

`G_new` stays **random** (mirrors genesis + preserves the ra7 free-migration model where group_id is never derived from the key). The phrase-IS-key invariant is untouched: `K_new` ⇒ `phrase_new = key_to_phrase(K_new)`.

## 4. New-key generation + the new phrase (UX)

- **Generate:** `K_new = GroupKey::random()`; `G_new = GroupId::new_random()`; derive `phrase_new = key_to_phrase(K_new)`, `D_new = derive_discovery_handle(K_new)`, `auth_new = derive_relay_auth_key(K_new, G_new)`. All from existing pure functions.
- **The old phrase is now dead for the live group.** After rotation the old 24 words no longer recover current notes (their `disc`/`group_id` are gone from the relay). **The user MUST re-record the new 24 words** — the old backup card is worthless and, worse, actively dangerous to keep (see §8, revoked-device honesty).
- **"Your recovery phrase changed" screen** (mirror Anytype copy posture, reuse the ra7 Show-phrase screen chrome from P0.3b/P0.3c):
  - Reveal `phrase_new` behind an explicit tap.
  - Copy: *"Your recovery phrase has changed. Write down these new 24 words and store them safely. Your OLD phrase no longer recovers your notes — destroy any copy of it."*
  - Confirmation gate before the rotation is allowed to *complete its teardown step*: require the user to re-enter 2–3 positions of `phrase_new` (Anytype-style), so rotation can't silently strand a user with an unrecorded phrase. (Gate the teardown, not the register — see §5; the new group must be live before we ask the user to confirm, so nothing is lost if they abandon at the confirm screen.)
  - Honest note in-copy for the leak case: *"Rotating protects notes created from now on. Anything already synced with the old phrase may have been read and can't be un-shared."*

## 5. State machine — the rotation sequence + every partial-failure arm

Rotation is driven by ONE initiating device `d1` (the one where the user taps "Rotate recovery phrase"). It is journaled locally so any crash resumes deterministically.

### 5.1 Local rotation journal (crash anchor)

`d1` writes a small journal file (e.g. `<mosaic>/.tesela/rotation.json`, atomic write) recording `{state, G_old, G_new, K_new (secret — same on-disk sensitivity as group_key.bin), key_epoch, ts}`. The journal is written **before any relay call** and advanced after each durable step. Presence of a journal at startup means "a rotation is in flight — resume from `state`." Absence means "no rotation pending." The journal holds `K_new` because a crash after `register-new` must be able to re-seed / switch without regenerating a different key (which would orphan the already-registered `G_new`).

`key_epoch` = a monotonic counter carried in the local identity (0 at genesis, +1 per rotation). **Client-side only — never sent to the relay** (see §9, zero-knowledge). It exists so `tesela-tp0.3` can reason about member staleness without retrofitting (§10).

### 5.2 Happy path (ordering is load-bearing: register-new FIRST, teardown-old LAST)

```
                 journal state        relay effect
GENERATE      →  "generated"          (none yet; K_new/G_new minted, journal persisted)
REGISTER_NEW  →  "registered"         POST /groups/{G_new}/register {auth_new, intent, disc:D_new}   → Inserted
SEED_NEW      →  "seeded"             PUT  /groups/{G_new}/snapshot  (full mosaic re-sealed under K_new)
SWITCH_LOCAL  →  "switched"           adopt(mosaic, {G_new, K_new}); key_epoch += 1  (d1 now lives on G_new)
CONFIRM       →  (gate)              user re-enters 2–3 words of phrase_new  (§4)
TEARDOWN_OLD  →  "torn_down"          retire G_old (§6) → cascade-wipes G_old's registration+ops+snapshots+disc+tokens
DONE          →  (journal deleted)   rotation complete
```

- **REGISTER_NEW must include `disc:D_new`** so `G_new` is recoverable-by-phrase from the instant it exists (re-adoption in §7 depends on it).
- **SEED_NEW re-encrypts client-side.** "Re-encrypt the old ciphertext" is impossible on the relay (zero-knowledge — it can't decrypt). Re-encryption is `d1` opening its local plaintext Loro state and re-sealing it under `K_new` as fresh `G_new` snapshots. After SEED_NEW, `G_new` is a complete encrypted replica; nothing from `G_old` is needed anymore.
- **SWITCH_LOCAL is the point of no return for `d1`** — after it, `d1` operates entirely on `(G_new, K_new)`.
- **CONFIRM gates TEARDOWN, not REGISTER.** If the user abandons at the confirm screen, `G_new` is already live and seeded and `d1` is already switched; only the old-group teardown is deferred. Re-opening the app resumes at TEARDOWN (or offers "finish rotation").

### 5.3 Why this ordering (crash-safety invariant)

**Invariant: at every intermediate crash point, at least one fully-usable group exists on the relay, and no committed content is ever lost.** Register-new-first + teardown-old-last guarantees it. The inverse ordering (teardown-first, forced by reusing `G_old`) violates it — see §3.

### 5.4 Partial-failure arms (resume by journal `state`)

| Crash after | Journal `state` | On restart, `d1` does | Old group | Notes |
|---|---|---|---|---|
| GENERATE, before REGISTER_NEW | `generated` | Register `G_new` (or **abort**: delete journal, stay on `K_old`) | fully live | Rotation is freely abortable up to SWITCH_LOCAL. |
| REGISTER_NEW, before SEED_NEW | `registered` | Re-run SEED_NEW (register is idempotent; re-issuing is a no-op) | fully live | `G_new` registered but empty — harmless orphan if aborted; scrub or age-out. |
| SEED_NEW, before SWITCH_LOCAL | `seeded` | SWITCH_LOCAL, then TEARDOWN (snapshot upsert is idempotent) | fully live | `d1` still on `K_old` locally; `G_new` is a ready replica. |
| SWITCH_LOCAL, before TEARDOWN_OLD | `switched` | Retry TEARDOWN_OLD | still registered | `d1` now on `G_new`; `G_old` is a scrubable orphan. If teardown unavailable (no admin/self-teardown right), mark `G_old` for age-out (§6) and clear journal. |
| during TEARDOWN_OLD | `switched`/`torn_down` | Retry DELETE; **treat 404 as success** (already gone) | gone/going | Teardown is a single idempotent delete. |

- **Idempotency is the safety net:** REGISTER_NEW, SEED_NEW, SWITCH_LOCAL (`adopt`), and TEARDOWN_OLD (DELETE→404-is-done) are each safe to re-run. Resume = re-drive from the recorded `state`.
- **Abort semantics:** before SWITCH_LOCAL the user may cancel; `d1` deletes the journal and any half-registered `G_new` orphan is left to scrub/age-out (it holds no content, or a `K_new` replica nobody adopted — harmless, and the relay can't read it). After SWITCH_LOCAL rotation is committed and only rolls *forward*.
- **`G_new` collision:** astronomically improbable (122-bit random UUIDv4). If REGISTER_NEW ever returns 409 for a freshly minted `G_new`, regenerate `G_new` (+`auth_new`, journal) and retry — do NOT adopt a colliding foreign group.

## 6. Retiring the old group on the relay — teardown ordering, auth, and old-key ciphertext

The bead's explicit question — old-key ciphertext on the relay: **re-encrypt, discard, or age out?** Decision, in priority order:

1. **DISCARD via teardown (recommended default).** Delete `G_old`'s registration; the FK `ON DELETE CASCADE` wipes its ops, snapshots, `disc` index row, and device tokens in one shot (`store.rs`). Nothing is lost — `G_new` already holds a complete `K_new`-sealed replica (SEED_NEW). This is also the strongest security outcome: it removes the residual old-key ciphertext a revoked/leaked-phrase holder could still fetch.
2. **AGE-OUT (fallback only).** If teardown auth is unavailable, leave `G_old` to expire via the relay's known-member TTL / snapshot-gated compaction. **Weaker** — old-key ciphertext stays fetchable for the TTL window, so the old key must be treated as *still-live-until-expiry* in the threat model. Acceptable only as a degraded mode; surface it to the user ("old data will be purged automatically within N days").
3. **RE-ENCRYPT in place — rejected / impossible.** The relay is zero-knowledge; it cannot decrypt to re-encrypt. "Re-encryption" only ever happens client-side and is exactly SEED_NEW. There is nothing to re-encrypt on the relay.

**Teardown authorization — proposed relay addition.** Teardown today is admin-only (`--admin-token`). Requiring an out-of-band admin token for a routine user action (especially on the CF Worker) is wrong. **Proposed:** add a **MAC-gated self-teardown** `DELETE /groups/{group_id}/register` authenticated by that group's own `auth_key` (i.e. proof-of-possession of `K_old` — which `d1` still holds through SWITCH_LOCAL, and can compute for `G_old` from the journal). Rationale that keeps it safe: **holding the group key already grants full read/write/DoS**, so letting a key-holder *retire its own group* adds no capability beyond what the key already confers. Admin-delete remains the backstop (operator hijack-recovery). Flag for Lead: this is a new relay endpoint on both `tesela-relay` and the CF Worker.

**Teardown vs. new-registration ordering (crash-safety between the two):** already fixed by §5 — teardown-old is the LAST step, after `G_new` is registered, seeded, and adopted. A crash between "new registered" and "old torn down" leaves both groups on the relay; `G_old` is a harmless scrubable orphan (revoked devices can still read its residual ciphertext until teardown completes — hence teardown should run promptly, not be deferred indefinitely; §8).

**CF Worker parity (prerequisite).** The Rust relay's cascade is DB-enforced. The spec REQUIRES the CF Worker teardown to wipe the **same** footprint — registration + `disc→group_id` index + ops + snapshots + device tokens for `G_old` — atomically. If the Worker's discovery index or op store is a separate DO/KV without an equivalent cascade, teardown that only drops the registration would leave an **orphaned `disc→G_old` mapping and readable old ciphertext**. This is the already-filed **"CF admin-disc-scrub"** deferred child; it is a **hard dependency of rotation on the Worker relay** and must land (with the self-teardown endpoint) before rotation ships to devices talking to the Worker.

## 7. Device re-adoption UX (each still-trusted device)

Re-adoption is the SAME recover-from-phrase flow, driven by the NEW phrase. **The new key must NEVER transit the old group** (a revoked device on `G_old` under `K_old` could read it — §8), so there is no silent push; re-adoption is a deliberate per-device action, exactly like initial pairing. This is the intended cost of not trusting the compromised channel (Anytype has the same posture).

- **Primary — enter the new phrase:** on each trusted device, "Re-enter recovery phrase" → `phrase_new` → `phrase_to_key` → `D_new` → `GET /discover/{D_new}` → `G_new` → `recover_pairing_from_phrase` builds the relay-only pairing code → existing `adopt` path → bootstrap from `G_new` snapshots. **Zero new adoption code** — this is `recover_pairing_from_phrase` verbatim against the new phrase.
- **Shortcut — QR on the same network:** `d1` can show a QR of the new relay-only pairing code (carries `K_new` + `G_new`) for a faster nearby re-adopt — the existing QR path carrying the new identity.
- **Loud prompt on the trusted device:** a device still on `(K_old, G_old)` that starts getting `401 "group not registered"` / `404` from the relay (because `G_old` was torn down) should surface *"This device may have been removed from the group. Re-enter your recovery phrase to reconnect."* — turning the lockout signal (§8) into a re-adoption CTA rather than a silent sync stall.
- **Local edits survive re-adoption:** a trusted device's un-synced local edits live as plaintext Loro state on disk under `K_old`; on adopt to `(K_new, G_new)` they are re-exported/re-sealed under `K_new` and authored into `G_new` (the normal deposit path after adopt). Nothing is lost — the local CRDT is the source of truth; only the seal key changes.

## 8. What a revoked device can still do with the OLD key (stated honestly)

Rotation excludes a device by making the key it holds worthless for the *live* group. It does **not** and **cannot** claw back what that device already saw. Precisely:

- **Reads of already-held plaintext — unavoidable.** A revoked device holds `K_old` and whatever it already decrypted (its local `.tesela` materialized notes + any cached ops/snapshots). It can read all of that forever. Rotation protects **future** content, not past. Symmetric-key rotation has **no retroactive forward secrecy** — this is fundamental, not a Tesela limitation.
- **Reads of residual relay ciphertext — until teardown.** In the window between REGISTER_NEW and TEARDOWN_OLD, `G_old`'s registration still exists, so a revoked device's `K_old`-derived `auth_key` still authenticates `GET /groups/{G_old}/ops` and can fetch old-key ciphertext still on the relay. **This is why teardown is part of rotation, not optional cleanup, and should run promptly.** After teardown (discard mode), the old registration is gone → `401 "group not registered"`; in age-out mode the residual stays readable until GC (degraded — §6).
- **Cannot read NEW content — the actual guarantee.** New content is sealed under `K_new`, deposited under `G_new`. The revoked device has neither and can derive neither: `D_new = HKDF(K_new)` needs `K_new`; it can't discover `G_new` without `phrase_new`. Once teardown completes, `/discover/{D_old}` (the only handle it can compute) 404s. **From the rotation point forward, the revoked device is fully excluded from new content.**
- **Can still write to `G_old` until teardown — irrelevant.** It could deposit garbage to the old group, but every trusted device has moved to `G_new` and ignores `G_old`. Teardown reclaims the storage and stops it.

**In-app honesty:** the leak-case copy in §4 must state that rotating does not un-share already-synced content. Do not imply rotation is retroactive.

## 9. A device OFFLINE through the entire rotation

`d_off` was a trusted device on `(K_old, G_old)`, offline for the whole rotation. On reconnect it still holds `K_old`, `G_old`:

- **It is locked out of `G_old`.** After teardown, its requests to `G_old` get `401 "group not registered"` (register endpoint) / `404` (discover `D_old`). This is the **loud** signal that drives re-adoption (§7). *This is the argument for prompt teardown (discard) over age-out:* age-out leaves `G_old` alive for the TTL window, so `d_off` would keep syncing the **stale** old group, silently diverging, never seeing new content until it too is locked out — a silent-divergence footgun. **Prefer discard so lockout is loud and immediate.**
- **It cannot auto-discover `G_new`.** The only handle it can compute is `D_old`; there is no `disc` chain from the old key to the new group (by design — that path would defeat rotation). Re-joining REQUIRES the user to enter `phrase_new` on `d_off` (§7). Until then `d_off` shows the "may have been removed — re-enter recovery phrase" state.
- **Its un-synced offline edits are preserved,** migrated on re-adopt exactly as in §7 (plaintext local Loro state re-sealed under `K_new`, authored into `G_new`). A device offline *through* a rotation loses no local work — it just has to re-adopt to ship it.
- **The multi-device honesty:** there is an inherent window where different devices are on different keys (whoever hasn't re-adopted yet). That's expected and safe — each device is either on `G_old` (locked out once torn down) or fully migrated to `G_new`; there is no state where a device silently reads a mix. The `key_epoch` counter (§5.1) lets a device tell "I'm behind" locally.

## 10. What `tesela-tp0.3` (multi-user) needs from this primitive

`tp0.3` introduces per-user / per-member keys and per-member revocation (the dormant Ed25519 device-identity columns in `schema.rs`/`group.rs` become load-bearing). This single-group full-rotation primitive is the **substrate + worst-case fallback** underneath it:

- **The crash-safe mint→register→seed→switch→teardown state machine (§5) is reused verbatim.** tp0.3's "remove a member" still rotates the underlying group key; only **key distribution** changes — from "re-enter the phrase on each device" to "re-wrap the new key to the *remaining* members' Ed25519 device pubkeys" so honest members don't have to re-onboard. The full-phrase re-adoption here is the v1 stand-in for that re-wrap.
- **The `key_epoch` counter (§5.1) is the forward-compat hook.** Introduce it now (client-side, monotonic) so tp0.3 can reason about "which epoch is this member on / is it stale" without a retrofit. It stays **client-side and never on the relay wire** — putting an epoch on the relay would let the relay link successive group_ids and observe rotations, breaking the §3 zero-knowledge win. tp0.3 keeps epoch out of the relay too.
- **The Ed25519 columns are tp0.3's, not this bead's.** This primitive must not wire them, and must not conflict: "re-adoption = re-enter phrase" is deliberately the identity-free v1 path, so tp0.3 can layer signed membership + key-wrapping on top without unwinding anything here.
- **Self-teardown auth (§6) generalizes.** tp0.3 will want membership-authenticated teardown/re-key operations; the proof-of-possession `DELETE /groups/{id}/register` here is the first instance of "a key-holder retires/mutates its own group," which tp0.3 extends to signed, per-member operations.

## 11. Non-goals (explicit)

- **No retroactive forward secrecy.** Rotation does not un-share content a revoked/leaked key already decrypted (§8). Fundamental to symmetric rotation.
- **No automatic / silent re-key push.** The new key never transits the old channel; every device re-adopts by entering the new phrase (or same-network QR). Deliberate (§7).
- **No per-member / per-user key rotation, no ACLs, no per-member revocation-without-global-re-key.** That is `tesela-tp0.3`. This primitive rotates the ONE group key for the whole group.
- **No Ed25519 device-identity wiring.** Dormant columns stay dormant here (`tesela-tp0.3`).
- **No relay-side epoch, linkage, or rotation signal.** The relay must stay unable to link `G_old → G_new` or tell a rotation happened. `key_epoch` is client-only (§9, §10).
- **No change to the phrase-IS-key invariant or the random-group_id model.** `K_new` still renders to a 24-word phrase; `G_new` is still random (not key-derived).
- **No re-encryption of old ciphertext in place on the relay** (impossible under zero-knowledge; SEED_NEW is the only re-encryption — §6).
- **No scheduled / automatic rotation.** v1 is user-initiated only (lost-device / suspected-leak remediation, or deliberate exclusion).
- **No relay-authority guarantees.** The relay stays a zero-knowledge mailbox; correctness of rotation rests on client-side journaling + idempotency, never on relay behavior beyond storing opaque bytes.

## 12. Open questions (resolve before/within impl)

- **Self-teardown endpoint vs. admin-only:** ship the proposed MAC-gated `DELETE /groups/{id}/register` (§6) so rotation self-serves teardown, or require the CF admin path for v1 and accept age-out when absent? (Recommendation: ship self-teardown — routine rotation should not need an operator token.)
- **Confirm-gate strength (§4):** re-enter 2–3 word positions of `phrase_new`, or a simpler "I've written it down" checkbox? (Recommendation: positional re-entry, Anytype-style, because an unrecorded new phrase after a completed rotation is unrecoverable.)
- **`key_epoch` surfacing:** purely internal, or shown in Settings ("Recovery phrase rotated N times / last rotated <date>")? (Advisory; a "last rotated" timestamp is good hygiene.)
- **Age-out window** when teardown is unavailable: what TTL, and how loudly to warn the user that old data lingers (§6)?

## Verify

None — design spec. The orchestrator Lead-reviews. (Implementation phases derived from this spec each carry their own `Verify:` — Rust `cargo test -p tesela-sync` / relay conformance for the relay pieces / `xcodebuild test` + a device round-trip for the UI, mirroring the ra7 P0 phasing.)
