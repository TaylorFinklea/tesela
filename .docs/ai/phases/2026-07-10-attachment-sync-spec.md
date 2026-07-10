# Cross-device encrypted attachment availability

**Bead:** `tesela-8zd.3` · **Tier:** Lead · **Status:** implementation spec only

## Problem and scope

Logseq import copies source `assets/` into the mosaic's `attachments/` folder
and rewrites `../assets/` links to `../attachments/`
(`crates/tesela-core/src/import_logseq.rs:471-500`, `:708-712`). The files
never traverse the Loro/relay path. Consequently, the current cross-device
system can converge note text while an iPhone shows broken images for the
imported asset corpus (about 94 files / 91 MB in the audit).

This spec decides how attachment bytes become available to every paired device.
It does not block independently planned web attachment view/paste routes; they
can proceed against the local `attachments/` contract. It covers byte transfer,
metadata, client materialization, storage limits, integrity, and restore.

## Decision: an encrypted, content-addressed R2 blob channel

Ship a dedicated attachment blob channel through the Cloudflare relay:

- **R2 holds sealed chunks.** The Worker is the only R2 caller; bucket objects
  are never public URLs. This is the byte store appropriate for the 91 MB
  corpus.
- **The per-group Durable Object remains the authorization/control plane.** It
  reuses the existing authenticated group route model; it does not become a
  binary store.
- **Loro carries metadata only.** The already-reserved `AttachmentUpsert` and
  `AttachmentDelete` variants say exactly this in
  `crates/tesela-sync/src/oplog/op.rs:156-178`: bytes flow out-of-band through
  a content-addressed blob store. This work completes that designed seam
  instead of creating a parallel metadata channel.
- **A client seals before upload and opens after download.** Existing relay
  envelopes already use XChaCha20-Poly1305 client-side
  (`crates/tesela-sync/src/crypto/aead.rs`) and the relay authenticates members
  with an HKDF-derived per-group key plus request MAC
  (`crates/tesela-sync/src/crypto/relay_auth.rs`). Blob requests use the same
  group authentication model and do not trust relay/R2 encryption as the
  confidentiality boundary.

This preserves the Mac-off invariant: after upload, any paired device can
retrieve an attachment without contacting its authoring device.

### Alternatives considered

| Option | Result | Reason |
| --- | --- | --- |
| Encrypted R2 blob store behind the existing relay | **Chosen** | Available while the authoring Mac is off; keeps ciphertext and object identifiers opaque; R2 holds large bytes while the Worker/DO enforces group authorization. |
| Durable Object/relay-op storage for bytes | Rejected | The current relay is a 16 MiB request envelope store (`TESELA_RELAY_MAX_BODY = 16777216` in `cloudflare-relay/wrangler.toml`), not a 91 MB binary store. Keeping byte chunks in a DO also makes retention/cost and hot-object behavior the wrong concern. |
| On-demand fetch from the authoring device | Rejected | Violates the locked Cloudflare mailbox topology and leaves iPhone rendering dependent on the Mac being awake/reachable. |
| Explicit v1 desktop-only image descope | Rejected | It fails the named Logseq-parity blocker: files and images must render on the iPhone. |

## Wire and persistence contract

### Metadata is authoritative through Loro

Use the existing attachment op family, making it operational rather than a
no-op. An attachment record must include its owning note, original filename,
MIME type, byte length, content BLAKE3, and a validated mosaic-relative
attachment path so `../attachments/...` Markdown resolves to the same local
materialization on every device. The current reserved payload has all but the
path; add only the field necessary to preserve nested attachment paths.

Metadata is published only after every sealed chunk is accepted by the relay.
A receiving client may see metadata before it has downloaded bytes, but never
before the author has made those bytes retrievable. Deleting a reference removes
metadata/local materialization; v1 does not eagerly delete the shared R2 object
because another note, a delayed device, or a backup may still need it. Remote
GC is a retention-aware follow-up.

### Opaque content addressing and sealing

The raw BLAKE3 is synchronization metadata inside the encrypted Loro stream;
it must not become an R2 key. Derive a domain-separated blob-id key from the
GroupKey and GroupId with HKDF, then HMAC the content BLAKE3 with that key.
The resulting opaque digest is the per-group content address. Identical bytes
deduplicate within one group but cannot be correlated by another group or the
relay.

Seal each plaintext chunk independently with XChaCha20-Poly1305 under a
separate domain-derived blob key. Its authenticated data binds the blob
protocol version, GroupId, opaque content address, chunk ordinal, and declared
plaintext length. A chunk cannot be substituted across groups, blobs, or
positions. The manifest records ordered chunk count/lengths and the file BLAKE3;
a receiver verifies AEAD for every chunk and the final file digest before making
the file visible at `attachments/`.

Chunk request bodies cap at 8 MiB, safely below the current 16 MiB relay body
limit after sealing/encoding overhead. Worker routes reject a larger body before
R2 write. The object protocol is immutable/idempotent: an already-present
address/chunk is a successful retry, not an overwrite. Group members already
have the content key; the integrity check detects corrupted or mismatched
ciphertext before materialization.

### Relay surface

Add authenticated group-scoped blob upload, existence-check, and download routes
beside the existing relay operations. Reuse the canonical MAC path/body binding
from `relay_auth`; do not invent unauthenticated R2 presigned URLs. The Worker
validates membership and 8 MiB request bounds, proxies sealed bytes to/from an
R2 binding, and never parses plaintext.

The Worker configuration gains an R2 binding and a staged migration/deploy
plan. The current `wrangler.toml` has only the two Durable Object bindings, so
the binding and operational bucket provisioning are explicit work, not an
assumption.

### Client behavior and iOS storage

Desktop/server materializes verified downloads in the existing mosaic
`attachments/` path. iOS treats attachments as a bounded, evictable cache under
its Application Support storage, not as an unbounded copy of every remote file.
Follow the existing Application Support/background-download pattern in
`app/Tesela-iOS/Sources/Data/TranscriptionStore.swift` after reading it; do not
put blob transfer logic in a view.

When a rendered attachment is referenced but absent locally, the client shows a
loading/retry state and schedules a fetch. It only exposes the final local file
after the authenticated decrypt-and-digest check. Cache eviction may remove
verified local bytes, never Loro metadata; opening the reference fetches again.
The implementation must report cache usage and a user-visible storage-limit
state rather than silently exhausting iOS storage.

### Backup and restore

Desktop backup already captures `attachments/` in
`crates/tesela-backup/src/archive.rs:11-17` and verifies file checksums on
restore. Keep that behavior: a restored local backup is immediately usable
offline. The Loro authority/attachment metadata is also restored with the
existing `.tesela/loro` capture. A device without local bytes reconstructs its
cache from the encrypted blob channel. Remote blob retention must therefore
outlive ordinary local cache eviction and normal backup/restore windows.

## Sequencing and acceptance gates

### 1. Lock attachment metadata and cryptographic vectors

**Work:** Complete the reserved attachment-op semantics in `tesela-sync`, add
the validated relative path, opaque-address derivation, chunk sealing/opening,
and deterministic test vectors. Mirror the existing AEAD and relay-auth
domain-separation patterns; do not reuse the envelope AAD unchanged.

**Acceptance:** Same-group clients derive the same opaque address for the same
file; different groups do not; swapped chunk/group/ordinal data fails to open;
metadata contains no plaintext bytes; an attachment op is no longer a no-op.

**Verify:** `cargo test -p tesela-sync`; `cargo clippy -p tesela-sync -- -D warnings`.

### 2. Add the authenticated Worker/R2 blob gateway

**Work:** Add the R2 binding, group-authenticated blob routes, maximum-body
enforcement, immutable retry semantics, and Worker tests/mocks following the
current `group-do.ts`/`handlers.ts` dispatch pattern. Read current Worker/R2
API documentation during implementation; do not buffer a whole attachment in
memory or depend on a public bucket URL.

**Acceptance:** Unauthenticated/cross-group reads fail; a <=8 MiB sealed chunk
round-trips; an oversized request is rejected; retrying a present chunk does
not replace it; Worker/R2 never receives plaintext.

**Verify:** `pnpm --dir cloudflare-relay exec tsc --noEmit`; `pnpm --dir cloudflare-relay exec wrangler deploy --dry-run`.

### 3. Implement Rust upload, download, and local materialization

**Work:** Extend the existing relay transport with the blob client operations,
stream/hash/seal on upload, authenticated download/open/final-digest validation,
and atomic local materialization under `attachments/`. Publish metadata only
after all chunk uploads complete. Reuse the current relay client's group auth
and error model; do not relay bytes through Loro envelopes.

**Acceptance:** Two independent local mosaics in one group converge attachment
metadata; the receiving one downloads and verifies the referenced bytes;
corrupt ciphertext never appears as a file; a 91 MB fixture is chunked rather
than sent as one request.

**Verify:** `cargo test -p tesela-sync -p tesela-server`; `cargo clippy -p tesela-sync -p tesela-server -- -D warnings`.

### 4. Bind clients to verified availability

**Work:** Expose attachment change/availability through the existing FFI and
client data layers. Add iOS cache management and the render-triggered
loading/retry state. Integrate the server/web local attachment route only at
its established view/paste seam; do not duplicate attachment logic in Svelte
components or SwiftUI views.

**Acceptance:** A paired iPhone can open a note with an imported image while
the Mac is off, sees a clear loading/error state during transfer, then renders
the verified image; cache eviction causes a safe refetch; storage exhaustion is
visible.

**Verify:** `cargo test -p tesela-sync-ffi`; `pnpm --dir web check`; `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`.

### 5. Prove backup, restore, and multi-device regression behavior

**Work:** Add end-to-end fixtures covering imported `../attachments/` links,
blob upload/download, offline local-backup restoration, and a receive-after-sync
path. Keep the current backup checksum assertion as the local authority test.

**Acceptance:** A backup restore retains attachment bytes and renders without
network; a clean second device reconstructs the same verified asset from R2;
metadata with absent/corrupt bytes never yields a misleading successful render.

**Verify:** `cargo test -p tesela-backup -p tesela-sync -p tesela-server && pnpm --dir web test:unit && xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`.

## Proposed implementation beads

These are triage recommendations only. This spec creates or claims none.

| Suggested bead | Scope | Depends on | tier_floor | complexity | Verify |
| --- | --- | --- | --- | --- | --- |
| `8zd.3a attachment metadata + crypto` | Operationalize `AttachmentUpsert`, path validation, opaque addressing, chunk AEAD, vectors. | — | lead | L | `cargo test -p tesela-sync` |
| `8zd.3b CF encrypted blob gateway` | R2 binding, authenticated Worker routes, size/immutability controls, deploy migration. | 3a contract | senior | L | `pnpm --dir cloudflare-relay exec tsc --noEmit` |
| `8zd.3c Rust blob transport/materializer` | Relay client blob operations, streaming transfer, atomic `attachments/` materialization. | 3a, 3b | senior | L | `cargo test -p tesela-sync -p tesela-server` |
| `8zd.3d iOS FFI + bounded cache` | Attachment-change bridge, Application Support cache, fetch/evict/error state. | 3a, 3c | senior | L | `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'` |
| `8zd.3e web/server availability integration` | Join existing attachment view/paste routes to verified local availability and status. | 3c | senior | M | `pnpm --dir web check && pnpm --dir web test:unit` |
| `8zd.3f backup + cross-device acceptance` | Restore/offline and two-device fixtures; remote-retention regression guard. | 3b–3e | senior | M | `cargo test -p tesela-backup -p tesela-sync -p tesela-server` |

## Out of scope

- Public/shareable attachment URLs, CDN delivery, browser-direct R2 access, or
  server-side plaintext inspection/transcoding.
- Mac-as-hub/on-demand author-device transport.
- New attachment authoring UX, camera picker, OCR, thumbnail generation, or
  document preview design.
- Immediate remote blob deletion/garbage collection; that needs a separate
  retention and restore-safety design.
- Changing the local Markdown attachment syntax or rewriting imported links.
- Cross-user ACLs, billing/quota product policy, and multi-mosaic blob sharing.
