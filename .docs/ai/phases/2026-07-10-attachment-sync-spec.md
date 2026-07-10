# Cross-device encrypted attachment availability

**Bead:** `tesela-8zd.3` · **Tier:** Lead · **Status:** revised implementation spec

## Review disposition

This revision adopts every blocker and major finding in the 2026-07-10 Sol
adversarial review. There are no contested findings.

- The approved encrypted, content-addressed Cloudflare R2 channel remains the
  decision, but now has a full Loro registry/container schema and FFI surface.
- Attachment path/reference semantics, causal deletion, corpus inventory and
  resumable upload are specified.
- `AttachmentManifestV1`, exact R2 layout, sealed wire body, limits, and
  verify-before-publish rules are fixed.
- Restoring to a sibling path must restore the GroupKey before an engine opens.
- V1 is explicitly Cloudflare-only; relay capability discovery prevents a
  Rust self-host from publishing metadata it cannot serve.

## Verified baseline

The Logseq importer copies `assets/` into `attachments/` and rewrites markdown
references to `../attachments/`; neither bytes nor attachment metadata travel
in current Loro relay updates. The corpus found by the audit is roughly 94
files / 91 MB.

`OpPayload` reserves metadata-only `AttachmentUpsert` and `AttachmentDelete`,
but `LoroEngine` currently handles both as no-ops. The actual Loro data model
already has an always-resident index document and a dedicated views registry,
so a dedicated attachment registry document follows an established pattern.

`tesela-8zd.1` is landed: the server has a traversal-safe
`GET /attachments/{path}` route and the web editor resolves relative image
URLs at render time. It is a local-materialization consumer, not a transport.
iOS has no equivalent attachment renderer/context yet; its capture attachment
control is a stub.

Backups include `attachments/`, Loro snapshots, and identity files, but the
macOS GroupKey store is scoped to the mosaic's absolute `.tesela` path in the
Keychain. Opening a restore at a sibling path without an explicit identity
restore can therefore mint a new GroupKey. The existing recovery phrase derives
the GroupKey, and `adopt_group_identity` writes an identity through the active
Keychain/file-store seam.

The Cloudflare Worker currently has Durable Object bindings only; it has no R2
binding or blob routes. The current Rust relay has no blob object store. Both
use the same MAC canonical-request shape. Cloudflare's current R2 API supports
`head`, conditional `put(..., { onlyIf })`, and strongly consistent reads after
a successful put; the Worker must use those semantics rather than assuming
blind overwrite is safe.

## Decision

V1 supplies attachments through a **Cloudflare-only, group-authenticated,
client-encrypted, content-addressed R2 channel**. R2 stores ciphertext only.
The Group Durable Object verifies group membership/MACs and authorizes access;
it never receives plaintext. A paired iPhone can fetch after the authoring Mac
is off.

The existing 16 MiB relay-envelope limit remains unrelated to blobs. Loro
carries only attachment registry metadata. The reserved attachment op family
is replaced by registry operations; it must no longer be a no-op.

### Alternatives rejected

| Option | Decision | Reason |
| --- | --- | --- |
| R2 behind the authenticated Cloudflare relay | **Chosen** | Mac-off availability, bounded Worker requests, opaque per-group storage keys. |
| Author-device fetch | Rejected | Violates the Cloudflare mailbox and Mac-off invariants. |
| Relay envelopes / DO SQLite for the 91 MB corpus | Rejected | Wrong body-size, storage, and retention model. |
| Desktop-only trial | Rejected | Fails the named iPhone image-rendering blocker. |
| Claim Rust relay parity without a blob backend | Rejected | Current Rust relay has neither R2 nor blob routes. |

## 1. CRDT registry and attachment semantics

### Dedicated attachment registry Loro document

Create one reserved, always-resident Loro document for attachment metadata,
parallel to the existing views registry rather than embedding attachment state
in per-note documents. It has `meta.schema_version = 1` and a `paths` map.
The registry is synchronized as a normal Loro document/snapshot and is exposed
by `tesela-sync` and FFI.

The **canonical validated relative path** is the `paths` map key. It is NFC,
uses `/`, contains no empty, `.` or `..` segments, and is relative to the
mosaic `attachments/` root. No Markdown path is changed; the existing local
route continues serving that same path after verified materialization.

Each `paths/<path>` entry is a Loro map with these fields:

| Field | CRDT shape | Meaning |
| --- | --- | --- |
| `manifest` | map, field-level LWW | Current `AttachmentManifestV1`; all fields are written together only after remote verification. |
| `refs` | map keyed by stable reference id | References from every live note to this path. |
| `refs/<ref>/adds` | append-only map keyed by add tag | Observed reference additions. |
| `refs/<ref>/removes` | append-only map keyed by add tag | Causal removals of observed additions. |
| `refs/<ref>/note_id` | scalar | Note owning this reference. |
| `refs/<ref>/source_path` | scalar | Canonical path, repeated for audit/debug validation. |

`ref_id = BLAKE3("tesela-attachment-ref-v1" || note_id || canonical_path)`;
there is one semantic ref per note/path even if the Markdown repeats the same
URL. `add_tag` is a new random UUID created for each add. A reference is live
when it has at least one add tag not present in `removes`. A delete records
only the add tags observed by that writer; it never deletes the map entry.
Thus a concurrent add survives a delete that did not observe it. This is the
required causal-delete behavior without relying on a map-key delete winning
against concurrent interior edits.

The registry projects a path's live-reference count from all refs. It may remove
the local materialized file only when that count is zero. V1 never deletes an
R2 blob, so it cannot delete bytes still needed by a delayed device or backup;
a future retention/GC worker must retain a blob whenever any live registry ref
remains and must additionally respect its recovery retention window.

### Path collision and multi-reference policy

A path is one registry identity. Multiple notes may reference the same path and
share its manifest/bytes. The importer inventory detects two different local
contents for one canonical path before it writes metadata. That is a blocking
attachment-path collision in the serialized import plan; the user must rename
or skip one source. It must not silently choose one file.

A legitimate later replacement of a path writes a new fully verified manifest
at the same path. All live refs for that path then see the new bytes, matching
the existing Markdown/path contract. A receiver never exposes a partially
changed file: it verifies the new manifest and materializes atomically.

### Engine and FFI records

Replace the reserved opaque payload with named attachment-registry operations:
add reference, causally remove observed add tags, and publish a verified
manifest. Add `AttachmentRecord`, `AttachmentManifestRecord`, and availability
records on the `tesela-sync` API and `tesela-sync-ffi` bridge. They include
path, owning note/reference ids, live-ref state, manifest, and local state
(`missing`, `downloading`, `verified`, `failed`, `unsupported_relay`).

The FFI is the iOS source of attachment state; it must not ask a Mac server to
resolve attachment metadata. Transfer/file I/O runs in a non-main-actor data
service, not a SwiftUI view or a `@MainActor` store.

## 2. AttachmentManifestV1 and crypto/wire contract

### Versioned manifest

`AttachmentManifestV1` is stored as fields in the registry `manifest` map and
is versioned independently of the Loro document schema:

| Field | Value |
| --- | --- |
| `protocol_version` | integer `1` |
| `path` | canonical relative attachment path |
| `mime_type` | detected MIME, not trusted extension alone |
| `plaintext_length` | exact final byte count |
| `file_blake3` | 32-byte BLAKE3 of plaintext file |
| `chunk_plaintext_bytes` | `4_194_304` (4 MiB) |
| `chunk_count` | exact count; `ceil(length/chunk_size)`, or 0 only for an empty file |
| `blob_id` | base64url HMAC-derived opaque group-scoped content address |
| `chunks` | ordered list of `{ index, plaintext_length, sealed_sha256 }` |

The receiver rejects unknown protocol versions, invalid path, count/length
inconsistency, a non-final short chunk, missing indexes, or an aggregate
plaintext length that does not equal `plaintext_length`.

The raw file BLAKE3 is encrypted metadata in Loro. It is never an R2 key.
Derive `blob_id` by HMAC-SHA256 over `file_blake3` with a domain-separated key
from `(GroupKey, GroupId, "tesela-attachment-address-v1")`. Identical bytes
deduplicate inside one group but do not create a cross-group object identifier.

For each chunk, derive a per-blob encryption key and a deterministic 24-byte
XChaCha nonce from GroupKey, GroupId, blob id, and chunk index under separate
`attachment-key-v1` / `attachment-nonce-v1` domains. Deterministic sealing is
intentional: two devices uploading the same content produce identical sealed
bytes for the same immutable object key. The AAD is the exact canonical binary
concatenation of:

```text
"tesela-attachment-v1\0" || GroupId(16) || blob_id(32) || chunk_index(u32-be) || plaintext_length(u32-be)
```

A ciphertext cannot be moved to another group, blob, chunk ordinal, or declared
length. The AEAD tag plus final file BLAKE3 are both mandatory before local
materialization.

### R2 layout and HTTP body

The immutable R2 key is:

```text
v1/groups/<group-id-hex>/blobs/<blob-id-base64url>/chunks/<index-u32-decimal>
```

No filename, MIME, plaintext hash, or note id appears in an R2 key. The
authenticated request is:

```text
PUT /groups/<group-id-hex>/attachments/v1/<blob-id-base64url>/chunks/<index>
```

Its raw body is exactly:

```text
"TSA1" (4 bytes) || version (u8 = 1) || nonce (24 bytes) || ciphertext_and_poly1305_tag
```

`MAX_PLAINTEXT_CHUNK_BYTES = 4_194_304`; the maximum sealed body is
`4_194_349` bytes (4-byte magic + 1-byte version + 24-byte nonce + plaintext +
16-byte tag). The client streams and counts plaintext while creating chunks;
it rejects a file that exceeds the product's configured attachment limit
before publication. The Worker rejects a missing/oversized `Content-Length`
before buffering and uses a counted body read that aborts if the cap is
exceeded. The existing `canonical_request` MAC signs the exact external path,
query, nonce, timestamp, and SHA-256 of these exact sealed body bytes.

### Conditional create and poisoned-object defense

For an upload the Worker:

1. MAC-authenticates the group request and validates all path components,
   index, `Content-Length`, wire header, and cap before R2 I/O.
2. Calls R2 `head`. If an object exists, it compares exact sealed length and
   stored `sealed_sha256` metadata with the manifest candidate; mismatch is a
   poisoned/collision error, never an overwrite.
3. If absent, uses R2 conditional `put` with `onlyIf` creation semantics and
   stores `sealed_sha256` as custom metadata. A failed condition re-runs step
   2; it does not retry an overwrite.
4. `head`s the successful/existing object and verifies the expected length and
   sealed digest before replying success.

R2's documented conditional `put` returns `null` on a failed precondition and
is strongly consistent after success; use that result explicitly. Metadata is
published to Loro only after every chunk has passed step 4.

Downloads use a MAC-authenticated GET under the same group/path scope. The
Worker streams the R2 body; the client writes only to a temporary cache file,
opens every chunk with the canonical AAD, validates every chunk and final
BLAKE3, then atomically renames to `attachments/<path>`.

## 3. Existing-corpus producer and availability

The 94-file corpus is not assumed to upload itself. After a successful
post-`tesela-ewj.1` Logseq import, and on later startup reconciliation, the
attachment producer:

1. scans the local `attachments/` tree safely, normalizes paths, streams each
   file's BLAKE3/length/chunks, detects MIME, and parses local note references;
2. builds the same registry ref set for every `(note_id, path)` and reports
   blocking path/content collisions in the import plan;
3. obtains relay capabilities before any upload;
4. probes every deterministic R2 chunk key, uploads only missing chunks, and
   verifies each result as above; a restart re-scans/probes, so no in-memory
   queue is required for resumability;
5. publishes the manifest plus reference adds only after the complete remote
   blob is verified; and
6. schedules download/materialization for metadata that is live locally but
   missing from the local cache.

The producer must be idempotent: rescanning an unchanged corpus adds no
reference tags and performs no overwrite. Failed uploads leave no manifest
published; UI reports a retryable availability error rather than a false
"synced" image.

The local server/web dependency is **`tesela-8zd.1` (landed)**. It consumes the
verified local path using its existing attachment route/render-time URL
resolution. The iOS renderer/cache is a required new dependency bead,
**`tesela-8zd.3e iOS attachment renderer + bounded cache`**, and cannot be
implicitly satisfied by `tesela-8zd.1`.

iOS keeps an evictable Application Support cache under the data layer. A
referenced missing image shows loading/retry/error, never a successful empty
render. Cache accounting and a user-visible storage limit are required; cache
eviction removes only verified local materializations and triggers a safe
refetch on next use.

## 4. Relay capability decision

V1 support is **Cloudflare relay only**. Add a small public
`GET /capabilities` response on both relay implementations with a versioned
feature list and `max_sealed_attachment_chunk_bytes`. Cloudflare advertises
`attachment_blob_v1`; the current Rust relay advertises no attachment blob
feature. A missing route/404 is capability false.

`RelayClient` checks capabilities before inventory/upload. If the active relay
does not advertise `attachment_blob_v1`, it must not publish new attachment
metadata or reference changes that require remote bytes; it reports a clear
unsupported-relay state. Receivers preserve already-synced metadata but show
unavailable rather than attempting an invented endpoint.

A future self-host feature may implement blob storage and advertise the same
capability only after it supplies equivalent authenticated routes, conditional
object semantics, retention, and conformance vectors. It is not part of this
bead and must not be presented as Rust-relay parity.

## 5. Backup/recovery and GroupKey restore

A local backup still includes `attachments/` and Loro authority, so restoring
its bytes can render offline. However, remote R2 blobs remain decryptable only
with the original GroupKey.

Restore adds a mandatory identity phase before any `load_or_create` engine
open at the destination path:

1. restore `group_id` from the backup authority;
2. obtain the original GroupKey through the age-protected identity envelope or
   the existing recovery phrase; and
3. call the existing identity-adoption path so the destination's active
   Keychain/file store contains the recovered `(GroupId, GroupKey)`.

A sibling-path restore must never silently mint a new key. If neither recovery
source is available, restore may recover local bytes but is marked
`remote_attachment_recovery_blocked` and must not start a sync engine that
would publish/consume blobs under the wrong key.

Add a restore-to-different-path drill: create/upload a fixture, back up,
restore under a distinct mosaic path, recover/adopt the original identity,
reopen, and prove the same group id/key derives the same blob id, opens a
remote chunk, and preserves Loro lineage. Also test the negative path: opening
without identity recovery is rejected before a new GroupKey can be used.

## 6. Worker implementation constraints and tests

Add an R2 binding through Wrangler configuration and generate Worker binding
types; do not hand-write an `Env` interface. The Worker/DO uses its binding,
not Cloudflare REST APIs or public/presigned URLs. Keep request-specific state
in handlers/DO instance calls, await every R2 promise, emit structured
non-secret observability events, and never log group keys, plaintext hashes,
or ciphertext bodies.

Workers-runtime tests use an actual R2 binding (Cloudflare's Workers Vitest
pool is the preferred harness) and cover:

- unauthenticated and cross-group GET/PUT rejection;
- missing/oversized body rejection before object storage;
- exact canonical MAC path/body mismatch rejection;
- first conditional put, same-object retry, competing/poisoned object mismatch,
  and post-put `head` verification;
- ciphertext corruption, wrong AAD/group/index, and final digest failure;
- capability false preventing metadata publication; and
- no unawaited transfer/R2 promises.

## Required acceptance and verify

- Two Loro engines converge the registry's manifest, concurrent distinct refs,
  and causal delete-vs-add outcome.
- A 91 MB fixture is streamed as 4 MiB chunks, not sent in a relay envelope.
- Import/startup inventory uploads the existing corpus idempotently and
  publishes no metadata before all chunks verify.
- A clean paired device with the Mac off obtains verified bytes and each client
  renders via its local web/iOS path.
- Backup restore at a different path preserves the GroupKey and decrypts an R2
  blob.

**Verify:**

```bash
cargo test -p tesela-sync -p tesela-sync-ffi -p tesela-server -p tesela-backup
pnpm --dir cloudflare-relay exec wrangler types
pnpm --dir cloudflare-relay exec tsc --noEmit
pnpm --dir web check
pnpm --dir web test:unit
pnpm --dir web test:e2e
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'
```

## Suggested implementation beads

These are planned dependencies only; this specification claims none.

| Suggested bead | Scope | Depends on |
| --- | --- | --- |
| `tesela-8zd.3a` | attachment registry CRDT, manifest v1, crypto vectors, FFI records | — |
| `tesela-8zd.3b` | Cloudflare R2 binding, capabilities, authenticated blob gateway, Worker-runtime tests | 3a contract |
| `tesela-8zd.3c` | Rust inventory producer, resumable transport, materializer, server availability integration | 3a, 3b, `tesela-8zd.1` |
| `tesela-8zd.3d` | backup identity envelope/recovery-phrase sibling-path restore drill | 3a |
| `tesela-8zd.3e` | iOS FFI consumer, bounded cache, relative attachment renderer | 3a, 3b, 3c |
| `tesela-8zd.3f` | two-device corpus/retention acceptance and product gate | 3b–3e |

## Out of scope

- Public URLs, browser-direct R2, server plaintext inspection/transcoding,
  attachment authoring/camera UX, thumbnail/OCR, immediate remote GC, sharing
  ACLs/billing, and Rust self-host blob storage.
