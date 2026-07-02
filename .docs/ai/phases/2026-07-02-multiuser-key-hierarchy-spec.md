# Multi-user key hierarchy — design spike (SPEC ONLY, no code)

**Bead:** `tesela-tp0.3` (multi-user key-hierarchy design spike, gates ALL Savanne work). **Type:** design spec only — no code, no schema migration, no wiring of the dormant Ed25519 columns.
**Epic:** `tesela-tp0` (minimum key/pairing model + Ed25519 device identity).
**Depends on (all design-approved / landing):** `tesela-ra7` P0 (recovery phrase = GroupKey, `disc` discovery handle, `/discover`, adopt), `tesela-tp0.1` (GroupKey rotation as a first-class op — `2026-07-01-groupkey-rotation-spec.md`; **rotation is the primitive this builds on**).
**Grounding:** `decisions.md` ADR-5 (2026-07-01) + 2026-06-30 recovery-phrase + 2026-07-01b ratifications; `crates/tesela-sync/src/crypto/*`, `group.rs`, `schema.rs`; Savanne-as-co-editor product lock (memory `project_savanne_collaborator`, decisions 2026-05-27).

> ⚠️ **This designs convergence-critical, E2E-crypto that gates multi-user.** Nothing here ships without TDD + an adversarial crypto-review gate (same bar as ra7 / tp0.1). This document prescribes **no code**; implementation phases derived from it read the named modules and mirror them. The goal of the spike is to answer ADR-5's open questions and produce a decisions.md ADR proposal for Taylor (§16) — **not** to authorize implementation.

---

## 1. Problem (why this gates Savanne)

Per ADR-5: the single symmetric **GroupKey** is a bearer secret shared verbatim by every device. It cannot express **per-user identity**, **per-member ACLs**, or **per-member revocation**. Verified facts behind that:

- **One key, one shared credential.** All content is sealed under one 32-byte symmetric `GroupKey` (`crypto/aead.rs`, XChaCha20-Poly1305). The relay auth credential `auth_key = HKDF(group_key, salt=group_id, "tesela-relay-auth-v1")` (`crypto/relay_auth.rs`) is **identical for every device** — the relay's `mac_gate` (`crates/tesela-relay/src/handlers.rs:696-772`) verifies one per-group key and **cannot tell one member from another**.
- **No cryptographic identity.** DeviceId is a non-cryptographic UUIDv7. The Ed25519 columns exist but are **dormant** — `device_self.ed25519_{pub,priv}key` and `group_members.ed25519_pubkey` (`schema.rs:48-63`), `GroupMember.ed25519_pubkey: Vec<u8>` "Empty in Phase 1" (`group.rs:37-40`). Nothing generates, stores, or verifies a real keypair.
- **No revocation short of global re-key.** `remove_peer` only drops LAN bookkeeping. A leaked phrase / lost device = permanent, unrevocable, full read+write membership. tp0.1 adds *whole-group* rotation (the minimum "kick a device" story) but that re-onboards **every** honest device by re-entering the new phrase.

**The retrofit trap (ADR-5's core reason to spike first):** if multi-user ships on the current single-symmetric-key model and identity/ACL is bolted on later, **every existing group must re-key** to gain per-member structure. Designing the hierarchy *before* Savanne ships lets migration be additive (§12) instead of a forced re-key of the world.

## 2. What multi-user actually requires (Savanne, grounded)

From the product lock (memory `project_savanne_collaborator`, decisions 2026-05-27): the modal case is **two people, low concurrency, one shared mosaic** (household recipes/planning) — not "one user, many devices." Concrete requirements this design must satisfy:

- **Invite a *person*** (Savanne, with her own device[s]) into an existing mosaic — not hand her the raw bearer key with no identity.
- **Attribution** — "who edited last" becomes desirable; each op should be attributable to a member, which needs a signing identity.
- **Revoke a person** — remove Savanne (breakup, lost trust, lost device) and stop her reading *future* content, without forcing Taylor's own honest devices to re-onboard from a phrase.
- **(Later) read-only sharing** — show someone a mosaic they can't write to. Not v1-critical but the model must have a place for it.

Out of scope for the product (so out of scope here): strangers editing someone else's mosaic; sub-mosaic / per-note ACLs; org-scale RBAC. Households + 2–3-person teams is the ceiling.

## 3. Core architecture — three layers, one unchanged

The design separates three concerns the current model fuses into one key:

1. **Content encryption — UNCHANGED.** Mosaic content stays sealed under **one symmetric ContentKey** per *key epoch* (today's `GroupKey`, still BIP39-renderable, still random `group_id`). CRDT op-logs must be decryptable+mergeable by every reader, so per-note or asymmetric content sealing is wrong; a single shared symmetric key is correct and efficient. **Multi-user changes key *distribution* and *authorship*, never the content cipher.**
2. **Identity — per-member Ed25519 (promote the dormant columns).** Each member holds a long-lived signing identity. This is what a roster lists, what signs membership changes, and what signs authored ops so honest peers can attribute + gate them.
3. **Distribution + authorization — a signed roster + key-wrapping.** The authoritative set of members (with roles + public keys) is a **signed, client-authored roster**; the ContentKey reaches members by being **wrapped** (encrypted) to their key-agreement public keys, never by re-typing the phrase.

Everything below is how layers 2–3 attach to layer 1 without disturbing it — and, crucially, **without the relay ever becoming an authority** (ADR-2: relay = mailbox, never authority).

## 4. Identity unit — device-keyed roster in v1, user-tier as the end-state

The dormant schema is **per-device** (`group_members` keyed by `device_id`, one `ed25519_pubkey` per device). Two shapes:

- **v1 (first shippable multi-user): per-DEVICE roster; a "user" is a display-name label grouping devices.** Every device = one Ed25519 identity = one roster entry with a role. Add a person = admit their device key(s); revoke a person = revoke all their device entries + re-key (§7). This matches the dormant columns exactly, needs no certificate chain, and is enough for "Taylor + Savanne, a few devices each."
- **End-state: per-USER identity key + device sub-keys (certificates).** A *user* has a long-lived identity keypair (the thing that "is Savanne"); each device carries a device keypair **certified by** the user key. Benefits: revoke one lost device without evicting the user; "invite Savanne" adds one user, her devices self-enroll under her cert. Additive over v1 — roster entries gain `user_id` + a device-cert field; no re-key to introduce it.

**Recommendation: ship v1 = device-roster, design the wire/roster schema to reserve the user tier** (a nullable `user_id` + optional device-cert slot), so the user tier layers on later without a re-key. Do **not** build the cert chain in v1 (over-build for a 2-person household). Rationale + trade-off go in the ADR (§16).

**Identity = a keypair PAIR, not one key.** Ed25519 signs (roster ops, authored ops); it is a *signing* key and must not double as a key-agreement key. Wrapping the ContentKey needs an **X25519** key-agreement key per member. Options: (a) a dedicated `x25519_pubkey` per member (cleanest — separates signing from KEX; needs a schema column beyond the dormant one), or (b) derive X25519 from Ed25519 via the birational map (one identity, the `age`/libsodium `crypto_sign_ed25519_pk_to_curve25519` shortcut). **Recommend (a)** for hygiene; note (b) as an acceptable space-saving fallback. Wrapping itself uses a vetted sealed-box primitive (candidates: the `crypto_box` crate = X25519 + XSalsa20-Poly1305, or `age` recipients) — **never hand-rolled**, mirroring the "pick a maintained crate, mirror `crypto/*`" posture. `chacha20poly1305`/`hkdf`/`hmac`/`sha2`/`bip39`/`rand` are already deps; Ed25519/X25519/crypto_box would be added.

## 5. The membership operations — this is the "wrapped keys vs re-key-on-change" answer

The bead's central question. The answer is **both, split by operation** (the industry-standard group-key split):

| Operation | Mechanism | Re-key? | Cost |
|---|---|---|---|
| **Add a member** | Wrap the *current* ContentKey to the newcomer's X25519 pubkey → publish the envelope (§10). Add their entry to the roster (signed by an admin). | **No.** | Cheap. The newcomer is *allowed* to read history, so giving them the live key is correct. No disruption to anyone. |
| **Remove a member (revoke)** | **Re-key**: rotate to `ContentKey_new` via tp0.1's mint→register→seed→switch→teardown state machine, then **re-wrap `ContentKey_new` to every *remaining* member's pubkey**. Remove their roster entry (signed). | **Yes — mandatory.** | Runs tp0.1 rotation. Honest members do NOT re-enter a phrase — they unwrap the new key from their envelope. |
| **Add a device to a member** | Wrap current ContentKey to the new device key; add its roster entry (self-signed under the user cert in the end-state; admin-signed in v1). | No. | Cheap. |
| **Rotate for hygiene / suspected leak** (no membership change) | tp0.1 rotation + re-wrap to all current members. | Yes. | tp0.1, unchanged. |

**Why removal MUST re-key (not just delete a wrapped envelope):** deleting a member's key envelope does nothing — they already hold `ContentKey_old` in memory and on disk. Symmetric-key revocation has **no retroactive forward secrecy** (identical to tp0.1 §8): rotation protects *future* content only; the removed member keeps everything they already decrypted. Stated honestly in-app, per tp0.1 §4/§8.

**tp0.1 is the substrate — reused verbatim.** tp0.1 §10 already names this: "tp0.3's *remove a member* still rotates the underlying group key; only **key distribution** changes — from 're-enter the phrase on each device' to 're-wrap the new key to the remaining members' Ed25519 device pubkeys.'" So tp0.3 = tp0.1's crash-safe rotation machine + a **wrapped-envelope distribution step** replacing the phrase re-adoption. The `key_epoch` counter tp0.1 introduces (client-side only, never on the relay) is exactly how a member reasons about "which epoch's key do I hold / am I stale."

## 6. Read vs write grants

Two orthogonal grants; the model must keep them separate:

- **READ grant = the ability to unwrap the ContentKey for an epoch.** Granted by wrapping the key to your X25519 pubkey (§5 add / re-key). Revoked by rotating the key and **not** re-wrapping to you (§5 remove). Binary per-epoch: you can read every epoch whose ContentKey you hold, nothing in epochs you don't. **No sub-mosaic read scoping in v1** (that needs per-space/per-note keys — explicit non-goal, §14). Read enforcement is **100% cryptographic key-distribution** — see §7.
- **WRITE grant = being a roster *writer* whose signed ops honest peers accept.** Each authored op is signed by the author's Ed25519 key; honest members verify the signature against their trusted roster and **drop ops from non-writers** (and, optionally, the relay drops them too — §7). A **read-only** member holds the ContentKey (can decrypt) but is not a roster writer, so honest peers reject anything they author.

**The honest limitation of "read-only":** a reader physically holds the *symmetric* ContentKey, so nothing cryptographically stops them from *producing* a well-formed encrypted op. Read-only is enforced by **author-signature gating** (honest peers + relay refuse ops not signed by a roster writer), **not** by withholding the content key. A separate write-only key does not help — CRDT ops must be mergeable by all readers, so the encryption key is necessarily shared. This is a real, must-be-documented property: **"read-only" is a social/authorship guarantee enforced by honest clients, not an information-theoretic one.** For the 2-person household it's fine; state it plainly rather than implying cryptographic write-exclusion.

**Roles (v1 minimal → end-state):**
- v1: `{ owner/admin, writer }`. Owner is the roster trust root (can admit/evict/rotate). Writer authors ops. (Collapse reader-only + multi-admin into "defer.")
- End-state: `{ admin, writer, reader }`, multi-admin allowed. A role that isn't enforced is a security lie — so **do not ship a `reader` role until author-signature gating enforces it** (§13 guardrail).

## 7. What a zero-knowledge mailbox CAN and CANNOT enforce

The bead asks explicitly. Grounded in the relay's actual surface (`mac_gate` verifies one shared per-group `auth_key`; the relay holds only opaque ciphertext + `disc` index + device tokens):

**CAN enforce:**
1. **Admission (coarse).** Possession of *a* valid group credential — today's single `auth_key` MAC. Keeps total strangers out. **Per-group, not per-member** — it cannot distinguish members. ✅ exists.
2. **Replay / rate limits.** Nonce dedupe, ±300s replay window, rate-gating (`mac_gate` + `rate_gate`). ✅ exists.
3. **Storage teardown / GC on proof-of-possession.** tp0.1's self-teardown `DELETE /register` MAC-gated by the group key. ✅ proposed in tp0.1.
4. **Write-authorization — ONLY as untrusted, best-effort defense-in-depth, and ONLY if handed the writer roster's PUBLIC keys.** Public keys are not secret, so giving the relay the set of authorized-writer Ed25519 pubkeys **preserves zero-knowledge** (content stays opaque). The relay could then reject `PUT /ops` not signed by a roster writer. ⚠️ **This is spam/DoS reduction, never a guarantee** — the relay is untrusted (a malicious operator can accept a bad op or drop a good one). New versioned surface; not smuggled into ra7/tp0.1 (§13).

**CANNOT enforce:**
1. **Read control — never.** Reading = decrypting fetched ciphertext. Anyone holding the ContentKey reads; a mailbox serving bytes cannot prevent it. **Read revocation is achievable ONLY by rotating the key** (§5 remove). The relay is irrelevant to read enforcement.
2. **Per-member distinction — not today.** The single shared `auth_key` makes every member look identical. Per-member relay auth (each member signs its requests with its own key against a relay-held pubkey roster) is *possible* and ZK-safe, but it's a **separable upgrade** to the bearer-auth layer, independent of content crypto. v1 can keep the shared `auth_key` and do all member-distinction client-side.
3. **Authoritative membership — never.** The roster is client-authored + client-verified. A malicious relay can drop/reorder/withhold roster ops or envelopes (a *liveness*/censorship attack) but **cannot forge a signed roster entry** (no member key). So even the relay-side write filter (CAN #4) is advisory; **clients always re-verify**. Authority lives in signatures, not on the relay.

**The crux, one sentence:** *the relay enforces admission + (optionally, best-effort) write-authorization; read control and true membership authority are purely client-side cryptography (key distribution + signature verification); the relay stays a zero-knowledge, untrusted mailbox — a convenience and DoS layer, never a security authority.* This is consistent with ADR-2 and every prior sync decision.

## 8. The roster — a signed, client-authored, convergent object

Membership state must sync like everything else (offline devices, concurrent admin actions), so the roster is **a CRDT-shaped object, but every mutation is signed**:

- **Entries:** `{ member_id/device_id, ed25519_pubkey, x25519_pubkey, role, added_by (admin pubkey), added_at, key_epoch_added, user_id? (reserved) }`.
- **Mutations are signed by an admin key** and verified by all. An unsigned or non-admin-signed roster mutation is dropped (like a non-writer op).
- **Convergence semantics mirror the note model:** additive union of adds; **removal-wins** (a signed removal beats a concurrent add/edit of the same member), echoing the block-delete "deleted-wins" rule (memory `project_block_delete_semantics`). Two admins concurrently admitting different members → union; concurrent admit+evict of the *same* member → evict wins. Deterministic, no split-brain.
- **Trust root / bootstrap:** genesis mints the founder as sole admin (§12). New admins are admitted by an existing admin. v1: single owner-admin is enough for a household; multi-admin is end-state.
- **Binding to re-key:** a `remove` that requires re-key (§5) is a two-part atomic-ish action — the signed removal + the tp0.1 rotation. Order: remove-from-roster is authored, then rotation runs and re-wraps only to the *post-removal* roster. A crash mid-way resumes via tp0.1's journal; the removed member is simply never re-wrapped to.

## 9. Where wrapped keys and the roster live (bootstrapping a joiner)

Key envelopes and the roster are small opaque blobs. Two delivery paths, both ZK-safe:

- **Invite path (out-of-band, no relay trust):** the newcomer generates their device keypair and presents their **public** keys (Ed25519 + X25519) to an admin — via QR / short code / a relay-mediated pubkey handle. The admin (a) admits them to the roster and (b) wraps the current ContentKey to their X25519 pubkey. The newcomer then fetches the roster + their envelope, unwraps, and joins. **This inverts today's one-way invite:** today the QR carries the raw ContentKey (`crypto/pairing.rs` — `group_key_bytes` in the blob = "possession = membership"). Real per-user identity requires the **newcomer's key to enter the roster**, so the invite becomes a **two-way handshake** (newcomer pubkey → admin approve). This is a genuine new pairing flow and a notable design consequence — call it out for Taylor (§15).
- **Re-key redistribution (relay-stored envelopes):** when an admin rotates + re-wraps to remaining members who are **offline**, each member's envelope is stored on the relay indexed by a hash of their pubkey and fetched by the member on reconnect. Envelopes are encrypted-to-a-pubkey, so relay storage is ZK-safe. (An offline member reconnecting after a re-key gets the tp0.1 "you may have been removed / re-sync" signal, then finds either a fresh envelope → unwrap → continue, or no envelope → they were the removed one.)

**v1 simplification worth considering:** keep the current ContentKey-in-QR invite for the *read* bootstrap AND have the newcomer self-enroll its pubkey into the roster with admin approval. Simpler, but still a handshake for the approval step. The tension (one-way blob vs two-way identity handshake) is the main UX cost of gaining revocable identity — flagged in §15.

## 10. Key wrapping mechanics (summary)

- **Wrap** = `seal(ContentKey)` to a recipient X25519 pubkey via a sealed-box primitive (ephemeral-sender ECDH → KEK → AEAD over the 32-byte ContentKey). One envelope per recipient per epoch.
- **Unwrap** = recipient uses its X25519 private key to recover `ContentKey`, then the existing `aead::seal/open` path is unchanged.
- **Epoch tagging:** each envelope carries its `key_epoch` (client-side; matches tp0.1's counter) so a member knows which epoch it just unwrapped and whether it's current.
- **No new content cipher:** once unwrapped, everything downstream (`aead.rs`, snapshots, ops) is byte-for-byte the current path. Wrapping is strictly a distribution wrapper around the *same* symmetric ContentKey.

## 11. Threat-model honesty (carry tp0.1 §8 forward)

- **Revoking a member protects future content only.** They keep what they already decrypted (local materialized notes, cached ops/snapshots) forever. No retroactive forward secrecy. In-app copy must not imply otherwise.
- **A leaked phrase is a read compromise of the current epoch.** Because the phrase *is* the ContentKey, anyone with it can read current content and (if honest-client gating is off, e.g. during migration) potentially author. Remediation = rotate (tp0.1) + re-wrap to the trusted roster; the leaked phrase's holder is not re-wrapped.
- **Phrase-recovery gives READ, not automatically WRITE.** A phrase-recovered device proves ContentKey possession (can decrypt) but its *new* device key is not yet a roster writer. For a solo user this should be frictionless (the phrase-holder is the owner → the recovered device self-admits as admin — a documented v1 convenience). For multi-user, a phrase-recovered device should be **confirmed into the writer roster by an admin**, so a leaked phrase alone doesn't silently become a writer. This cleanly separates the two revocations: rotate = kill read; roster-evict = kill write.
- **The relay can censor but not forge.** A hostile relay can withhold roster ops / envelopes (liveness attack, detectable as "I'm not receiving updates") but cannot fabricate a signed membership change. Design assumes an untrusted relay throughout.

## 12. Migration for existing single-user groups (free, additive, no forced re-key)

Existing groups have one ContentKey (the phrase), N of Taylor's devices sharing it, no Ed25519, no roster. Migration must **not** re-key and must not strand an un-upgraded device:

1. **Self-identity (local, no coordination).** On upgrade, each device generates its Ed25519 + X25519 keypair, filling the dormant `device_self` columns. No network effect.
2. **Bootstrap the roster from existing phrase-holders — not a downgrade.** In the pre-migration world, **possession of the phrase already IS full membership** (the current threat model). So seeding the roster by trusting current phrase-holders crystallizes existing implicit membership into explicit membership; it grants nothing new. The device where the user runs "Enable sharing" writes the initial roster (itself as owner/admin). Each of the user's other devices enrolls its device key (auto-adopted during a transition window because it demonstrably holds the phrase/ContentKey, or via an explicit "this is my device" tap).
3. **No re-key at rest.** ContentKey unchanged; the phrase keeps recovering read; roster + device keys layer on top. The **first *removal*** is the first event that triggers a re-key (§5).
4. **Compat ratchet (secure rollout).** Un-upgraded devices don't sign ops or understand the roster. Honest new devices must **accept unsigned ops during a grace window** so Taylor's old phone keeps syncing. Enforcement (drop unsigned/non-writer ops) turns on only when an admin explicitly **"locks the group to signed edits"** — a one-time, user-visible action taken *after* all devices are upgraded + enrolled. Do not auto-enforce; a premature lock silently drops a lagging device's edits. This is the standard "enforce only once everyone's migrated" ratchet.

**Net:** existing solo groups migrate with zero re-key and zero data movement; the ContentKey and phrase are untouched; identity is purely additive. The first real cost (a re-key) is paid only when a member is actually removed.

## 13. What ra7 / tp0.1 P0 crypto MUST NOT do meanwhile (guardrails)

The bead requires this list explicitly. ra7 (recovery phrase) and tp0.1 (rotation) are landing *before* multi-user; they must not foreclose this design:

- **MUST NOT treat "phrase possession = permanent authorization" as anything but a documented v1 read-bootstrap assumption.** The phrase gates *read*; do not build any feature that makes the phrase the sole/permanent *write* authority in a way roster-signature gating can't later supersede.
- **MUST NOT wire the dormant Ed25519 columns for any semantics.** (tp0.1 §10 already forbids this.) `device_self` / `group_members` Ed25519 stay dormant until tp0.3 defines their meaning. ra7 MUST NOT start signing the pairing blob or any wire with a device key in a shape tp0.3 would have to unwind.
- **MUST NOT put any member/device-identifying or roster data on the relay wire — nor any per-user handle that links key epochs.** (Extends tp0.1's "no relay-side epoch/linkage.") The relay's view stays: opaque ciphertext + one bearer `auth_key` + `disc`. When tp0.3 adds per-writer relay auth or a writer-pubkey filter, it enters as its own versioned surface — never smuggled in now.
- **MUST NOT change content encryption away from the single symmetric ContentKey.** tp0.3 wraps the *symmetric* key; keep content sealed under one symmetric key (no per-note/per-space keys, no asymmetric content sealing).
- **MUST NOT ship an ACL/role/read-only concept half-wired.** A role that isn't enforced by author-signature gating is a security lie. No `reader`/`writer` flags anywhere until tp0.3 enforces them.
- **MUST keep the invariants tp0.3 relies on:** phrase-IS-key, random `group_id`, free migration, zero-knowledge relay. The ContentKey stays BIP39-renderable; wrapping is a layer *over* the same key.
- **MUST keep tp0.1 re-adoption pluggable.** tp0.1 already frames "re-enter phrase" as the v1 stand-in for "unwrap to remaining members." Reaffirm: do not hard-code that full-phrase re-entry is the *only* re-adoption path — leave the seam for wrapped-envelope re-adoption.
- **MUST keep tp0.1 self-teardown auth as group-key proof-of-possession (not device-key) for v1**, so tp0.3 can generalize it to per-member-signed teardown/re-key without a breaking change (tp0.1 §10 flags this).

## 14. Non-goals (explicit)

- **No sub-mosaic / per-note / per-space read scoping.** One ContentKey per epoch covers the whole mosaic. Granular read control (per-note keys) is a separate, later epic.
- **No org-scale RBAC / permission matrices.** Roles cap at `{admin, writer, reader}` for households + small teams.
- **No cryptographic write-exclusion of a reader.** "Read-only" is honest-client author-signature gating, not information-theoretic (§6).
- **No retroactive forward secrecy** (§11) — fundamental to symmetric rotation.
- **No relay authority.** The relay never adjudicates membership; it stores opaque bytes and, at most, best-effort filters writes by a public roster (§7).
- **No user-tier certificate chain in v1** (device-roster only; user tier is reserved-but-deferred, §4).
- **No implementation, no schema migration, no wiring** in this bead — spec + ADR proposal only.

## 15. Open questions (resolve before/within impl)

1. **Identity unit for v1:** device-roster (recommended, matches dormant schema) vs go straight to user-tier certs? (Recommend device-roster; reserve `user_id`.)
2. **Invite handshake:** accept the two-way handshake (newcomer pubkey → admin approve) as the cost of revocable identity, or keep a one-way ContentKey-in-QR read-bootstrap + a separate admin-approval step (§9)? (Recommend the handshake for the real member; a one-way read-only share could keep the simpler blob later.)
3. **X25519 key:** dedicated per-member X25519 column (recommended, hygiene) vs derive-from-Ed25519 birational (one column)? 
4. **Per-member relay auth:** upgrade the shared `auth_key` to per-member signed relay auth (enables relay-side write filtering + per-member admission), or keep the shared bearer credential and do all member-distinction client-side in v1? (Recommend shared credential v1; per-member relay auth is a separable later hardening.)
5. **Compat-ratchet default:** how long is the unsigned-ops grace window, and how loudly is "Lock to signed edits" surfaced (§12)?
6. **Wrapping primitive:** `crypto_box` vs `age` recipients vs another vetted sealed-box crate (§10) — an impl-phase call, but name the shortlist now.
7. **Roster storage form:** a dedicated Loro object vs a signed-op stream vs a relay object — where does the signed roster physically live and sync (§8)?

## 16. decisions.md ADR PROPOSAL (for Taylor)

> Draft for Taylor to ratify. On approval, append to `.docs/ai/decisions.md` as an ADR (dated), and file the implementation epic/beads referencing it. **This spike does not authorize implementation** — multi-user stays gated on this ADR being ratified.

**Proposed ADR — Multi-user key hierarchy: symmetric ContentKey + per-member Ed25519 roster + key-wrapping; add=wrap, remove=re-key.**

- **Content stays one symmetric ContentKey per epoch** (today's GroupKey, still phrase-renderable, still random group_id). Multi-user changes *distribution* and *authorship*, never the content cipher.
- **Identity = per-member Ed25519 (sign) + X25519 (wrap).** v1 ships a **device-keyed roster** ("a user" = a labeled cluster of devices); the wire/roster schema **reserves a `user_id` + device-cert slot** so a per-user identity tier layers on later **without a re-key**. Promote the dormant Ed25519 columns; add an X25519 key.
- **Membership ops split:** **ADD = wrap the current ContentKey to the newcomer (no re-key).** **REMOVE = re-key via tp0.1 rotation + re-wrap to the remaining members (mandatory re-key; honest members unwrap, they do NOT re-enter the phrase).** tp0.1's crash-safe state machine is the substrate; the wrapped-envelope step replaces phrase re-adoption.
- **Read vs write:** **read = holding/unwrapping the ContentKey** (revoke read ⇒ rotate). **write = a roster *writer* whose signed ops honest peers accept.** "Read-only" is honest-client author-signature gating, **not** cryptographic write-exclusion — documented as such.
- **Relay stays a zero-knowledge, untrusted mailbox.** It enforces **admission** (bearer `auth_key`) + replay/rate limits, and MAY do **best-effort write filtering** if given the writer roster's *public* keys (ZK-safe). It **cannot** enforce read control or adjudicate membership — those are client-side signatures + key distribution. No member/roster/epoch-linking data on the relay wire.
- **Migration is additive, zero forced re-key:** existing solo groups generate device keys, bootstrap a roster from existing (already-fully-trusted) phrase-holders, keep the ContentKey + phrase untouched; a **compat ratchet** accepts unsigned ops until an admin explicitly "locks to signed edits" post-rollout. The first re-key is paid only on the first member *removal*.
- **Guardrails on ra7/tp0.1 (must-not, §13)** are ratified as constraints: Ed25519 columns stay dormant, no relay-side identity/linkage, content stays single-symmetric, no half-wired roles, tp0.1 re-adoption stays pluggable, tp0.1 self-teardown stays group-key-proof-of-possession.
- **Non-goals locked:** no per-note read scoping, no org RBAC, no user-cert chain in v1, no retroactive forward secrecy, no relay authority.

**Open decisions requiring Taylor's call before the impl epic:** §15 items 1 (identity unit), 2 (invite handshake vs one-way blob), and 4 (shared vs per-member relay auth) are the product/architecture-shaped ones; 3, 5, 6, 7 are impl-phase calls.

## Verify

None — design spec (bead Verify = human/Lead review, no build gate). Implementation phases derived from this spec + the ratified ADR each carry their own `Verify:` (Rust `cargo test -p tesela-sync` for crypto/wrapping/roster; relay conformance for any relay surface; `xcodebuild test` + a device round-trip for pairing/UX), mirroring the ra7 / tp0.1 phasing and the TDD + adversarial-crypto-review gate.
