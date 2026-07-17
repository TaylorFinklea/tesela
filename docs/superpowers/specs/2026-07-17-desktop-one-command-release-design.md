# Desktop one-command release (TestFlight parity) — Design

Date: 2026-07-17 · Bead: tesela-qkwd (P0) · Approach approved by Taylor.

## Goal

`bws-project run -- scripts/desktop-release.sh` publishes a tested desktop
release end-to-end, exactly like `scripts/ios-testflight.sh` does for
TestFlight. A second Mac installs the zip once from GitHub Releases; later
releases arrive via the built-in tauri-plugin-updater (endpoint:
`releases/latest/download/latest.json`).

Also ships the corrected self-contained release (v0.1.3) superseding the
white-screening v0.1.2 (fix `adf8f3bf` landed after v0.1.2 shipped).

## Non-goals

- No CI release path (secrets stay in Bitwarden / local `bws-project`).
- No beta/stable channel split (single tester).
- No Intel/universal builds (both Macs are Apple Silicon, darwin-aarch64).

## Changes

### 1. Version + changelog

- `src-tauri/tauri.conf.json` version `0.1.2` → `0.1.3`.
- `release-notes/releases.json`: new entry `2026-07-17.desktop-0.1.3`
  (platforms `["desktop"]`, `versions.desktop: "0.1.3"`, fixed: self-contained
  packaging / white-screen), `current.desktop` → that id.
- Mirror to `app/Tesela-iOS/Sources/Data/ReleaseNotes.json`
  (`check-release-notes-drift.sh` enforces byte equality).

### 2. `scripts/desktop-release.sh` flags

- Default: full pipeline **including publish + post-publish verify**.
- `--no-publish`: stop after `latest.json`; print `gh` commands (old behavior).
- `--skip-notarize`: unchanged; never publishes.
- `--dry-run` (new; bead `verify_cmd`): plan-only. Validates changelog/version
  alignment and tooling (`gh auth status`, `cargo tauri`, node); no build, no
  secrets, no publish; exit 0.

### 3. Publish stage

- Tag absent: `gh release create v$V <zip> <tar> <latest.json> --title v$V
  --notes-file <notes.md> --latest`.
- Tag present: `gh release edit --notes-file` + `gh release upload --clobber`
  (idempotent retry), and enforce `--latest`.
- `--latest` is explicit because date-based CLI releases share the repo and
  would otherwise steal `releases/latest/…` from the updater endpoint.
  Follow-up bead: CLI release workflow passes `--latest=false`.
- Monotonic guard: fetch live `releases/latest/download/latest.json`; abort
  unless new version is strictly greater (the v0.1.2 lesson — installed
  clients only react to a version bump; never republish in place).

### 4. Post-publish verify stage

In a temp dir, against the **published** artifacts:

- `gh release download v$V` the zip; extract.
- `assert_desktop_web_bundle` (Contents/Resources/web/index.html present).
- `codesign --verify --deep --strict`, `spctl -a -vv`, `stapler validate` on
  the downloaded app.
- Launch its binary directly with `TESELA_MOSAIC=<temp mosaic dir>` (avoids
  the installed app's single-writer flock); discover the ephemeral port via
  the `pgrep`/`lsof`/`/health` pattern from `install-desktop.sh:102`; assert
  `GET /g` → HTTP 200; kill the process.
- Assert live `latest.json` version == `$V` and its `signature` equals the
  local `.sig` contents.

### 5. Error handling

Publish-then-verify: a failed verify leaves the release live briefly; the
script exits non-zero and prints the rollback
(`gh release delete v$V --cleanup-tag --yes`). Draft-first was rejected:
drafts do not serve `releases/latest/download/` URLs, so the real updater
path could not be verified.

### 6. Docs + rehearsal

- New `docs/desktop-install.md`: first install on another Mac (download zip →
  unzip → drag to /Applications; updates are automatic afterwards).
- Human rehearsal (bead acceptance): open the installed v0.1.2 app; it must
  self-update to v0.1.3 — the true end-to-end updater + signature test.

## Verification

- `scripts/desktop-release.sh --dry-run` exits 0.
- `node --test scripts/tests/changelog.test.mjs` green;
  `scripts/check-release-notes-drift.sh` green.
- Real release run passes its own stage-4 verify.
- Human: installed 0.1.2 auto-updates to 0.1.3.
