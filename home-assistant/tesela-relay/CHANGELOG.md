# Changelog

## 0.1.0 — initial release

- First public Home Assistant add-on build of `tesela-relay`.
- Multi-arch (amd64, aarch64) image pushed to ghcr.io.
- Bind on `0.0.0.0:8484`, SQLite under `/data/relay.sqlite`.
- Options: `admin_token` (required), `max_body`, `log_level`.
