# Changelog

## 0.2.2 — stop serving the group auth_key from registration endpoints

- `GET /groups/:id/registration` (an open, unauthenticated endpoint) and
  the `POST /register` 409 conflict echo no longer include `auth_key_b64`.
  The auth_key is the MAC key gating every write — serving it openly let
  anyone who learned a group id mint valid request MACs. Every member
  derives it locally via HKDF from the group key instead; joiner
  verification still works from the public fields (registered_at + intent).

## 0.2.1 — fix op seq allocation after full compaction

- `insert_op` now allocates `MAX(MAX(seq), compaction_seq) + 1` instead of
  `MAX(seq) + 1` from the op table alone. After a snapshot deposit covering
  every op (full compaction empties `relay_ops`), the next PUT was assigned
  seq 1 — below every caught-up consumer's cursor — so the edit was never
  delivered and the next deposit deleted it permanently (the #195 black
  hole). Seqs now always advance past the compaction watermark.

## 0.1.1 — raise default max_body to 16 MiB

- `max_body` default 5 MiB → 16 MiB. A single Loro doc can't be split
  across relay envelopes, so the cap must exceed the largest note's
  snapshot on the wire (the biggest real note, ai-business, is a ~5 MB
  snapshot ≈ 7 MB encoded). The old 1 MiB / 5 MiB caps rejected it with
  413, jamming cross-device sync. Existing installs must raise `max_body`
  in the Configuration tab — the new default only applies to fresh installs.

## 0.1.0 — initial release

- First public Home Assistant add-on build of `tesela-relay`.
- Multi-arch (amd64, aarch64) image pushed to ghcr.io.
- Bind on `0.0.0.0:8484`, SQLite under `/data/relay.sqlite`.
- Options: `admin_token` (required), `max_body`, `log_level`.
