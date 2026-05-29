# Current State

## 2026-05-29 — Loro cutover FINISHED; redesign is next

**Loro is the sole sync engine.** Flag-day + ai-business dedup + DR drill all done, committed, green. Full report: `phases/2026-05-29-loro-cutover-report.md`.

### Commits this session
- `8ef366e` perf(sync): dedup — store frontmatter-only on root meta (lean snapshots).
- `471d619` refactor(sync)!: delete SqliteEngine/DualEngine/op-wire — Loro-only (~3.6k lines deleted).
- `c626d25` build(ios): regenerate UniFFI bindings for the Loro-only FFI.

### Build status
- `cargo build --workspace` + `cargo test --workspace` → GREEN (0 failures).
- `xcodebuild -scheme Tesela -sdk iphonesimulator` → **BUILD SUCCEEDED** (against the rebuilt `.a` + regenerated bindings).
- `cargo install` of `tesela-server` + `tesela` (flag-day binaries) — see this session's install.

### What the flag-day did
- Deleted `sqlite_engine.rs`, `dual_engine.rs`, `tests/convergence.rs`, `examples/two_node.rs`.
- `SyncEngine` trait = Loro-only (dropped `apply_changes`/`produce_changes_since`/`produce_local_authored_since`/`uses_loro_relay_payload`/`ProducedBatch`). Deleted the v1 op-wire (`encode/decode_op_batch`).
- Server: `main.rs` builds a bare `LoroEngine` unconditionally (no `TESELA_LORO_DUAL_WRITE`/`AUTHORITATIVE`; `TESELA_LORO_RESEED` kept for one-time canonical bootstrap). `sync_relay.rs` = Loro v2 only. Deleted dual-write divergence endpoints (kept `/loro/index`).
- **LAN P2P (peer_sync) data-plane RETIRED** — op-replay is incompatible with Loro + fully redundant with the relay spine; `produce`/`receive_envelope` → 501, daemon = no-op, pairing/discovery stay live. Follow-up: reimplement over the Loro relay-update protocol.
- FFI: `open_loro` is the sole constructor; ticks = Loro v2 only.

### Server launch (CHANGED — no flags)
`tesela-server --mosaic "/Users/tfinklea/Library/Application Support/tesela/logseq"` — Loro is now the default engine. Add `TESELA_LORO_RESEED=1` ONLY for a one-time canonical bootstrap from disk (one device). (No server is currently running.)

### DR drill (validated on an isolated copy — non-destructive)
Restore from `notes/*.md` + `TESELA_LORO_RESEED=1` rebuilds all 514 notes; `/health` 200, `/loro/index` = 514. **Dedup payoff: ai-business snapshot 5.13 MB → 2.58 MB** (now under the 5 MB relay limit). Canonical DR = the `.md` files are truth; `.tesela/loro/` is a derived cache.

## Blockers / open
- **Live data reset is USER-COORDINATED (needs the iPhone).** The dedup's size win lands only on fresh docs; the live mosaic still holds bloated snapshots, so ai-business won't sync until a coordinated reset: stop server → backup → `rm -rf <mosaic>/.tesela/loro/` → boot with `TESELA_LORO_RESEED=1` → **wipe + re-bootstrap the iPhone's local docs** (else fresh-identity docs duplicate against its old docs). Until then the server runs fine on existing docs via the backward-compat fallback (ai-business simply stays unsynced, as before). See the report's "Remaining" section.
- Backlog (unchanged): deferred review findings #7/#8 (slug-rename orphans), #10–18; #111 oplog-order (moot post-flag-day).

## ACTIVE MILESTONE — Graphite redesign (foundation DONE; shell next)
Approved spec: `phases/2026-05-29-graphite-redesign-spec.md`. Foundation plan: `phases/2026-05-29-graphite-foundation-plan.md`. Brand-new web (SvelteKit) + iOS (SwiftUI) frontends to the Graphite design system, reach daily-driver parity, then delete the old. Phasing: foundation → shell → daily-driver views → cutover → iterate. Web + iOS in parallel; shared tokens; REUSE vetted lib logic (CodeMirror editing engine) + Loro FFI/MosaicService. Design source: `.docs/ai/design/graphite/`.

### Foundation phase — LANDED (2026-05-29)
Executed subagent-driven (web + iOS in parallel). Commits: `7083956` (tokens.json + web primitives), `e316a6f` (iOS theme + SwiftUI primitives).
- **Shared tokens:** `.docs/ai/design/graphite/tokens.json` (canonical). Web: `web/src/lib/graphite/tokens.css` (mockup's exact `--*` vars scoped to `.gr-root`). iOS: `.graphite` Theme case in `Sources/DesignSystem/Theme.swift` (reuses existing `@Environment(\.theme)` infra).
- **Primitives (both platforms):** GrIcon (web=`@tabler/icons-svelte` name-map; iOS=Tabler→SF-Symbol map), GrButton (ghost/cta), GrChip, GrTypeDot, GrTypeTag, GrRow, GrWidget. Web in NEW `web/src/routes/g/` tree + `lib/graphite/` (old v4/v5 untouched). iOS in NEW `Sources/Graphite/` (Data/Sync/Generated/Views/Components untouched). Components reference tokens only (theme-swappable, no hardcoded hex bar the on-coral CTA ink).
- **Gates (re-verified by me):** web `svelte-check` clean for graphite (lone error = pre-existing v4 VoiceCaptureButton); iOS `xcodebuild -sdk iphonesimulator` → **BUILD SUCCEEDED**. (IDE SourceKit shows false-positive cross-file errors under this project's no-explicit-modules config — xcodebuild is authoritative.)
- **One deferred check:** visual parity of the `/g` primitives gallery vs the mockup screenshots — not done (user's Chrome held the MCP profile; build/type gates pass). View at `localhost:<webdev>/g`; iOS gallery via `GrGalleryView` `#Preview`. Confirm at shell-phase start.

### Shell phase — LANDED (2026-05-29)
Plan: `phases/2026-05-29-graphite-shell-plan.md`. Executed subagent-driven (web + iOS parallel). Commits: `88e4dfe` (web shell), `c897b98` (iOS shell). **All NEW Graphite presentation bound to EXISTING behavior — no behavior rebuilt.**
- **Web** (`web/src/lib/graphite/shell/`, composed at `/g`; primitives gallery moved to `/g/primitives`): GrTopBar (workspace tabs via `getWorkspace`/`switchTab` + ⌘K bar→`openStation` + connection dot via `getConnected`), GrRail (widget host: Quick-capture→`openColonMode`, Pinned=`getFavorites`, Today=`getRecents`, Tasks=placeholder, +Add-widget stub), GrPane (first-class, focus/side splits-ready, placeholder body), GrStatus (`getVimMode` + breadcrumb + clock), GrCommandPalette (mirrors `Station.svelte` over the real `buildV4Commands` + `scoreFuzzy`), GrLeaderOverlay (mirrors `ChordMenu` over `getLeaderTree`), GraphiteShell (composes + mirrors `v4/+layout` capture-phase keydown Space/⌘K/`:`).
- **iOS** (`Sources/Graphite/Shell/`): GrAppShell mirrors AppShell's native tab bar (4 tabs + `.search` glass circle) bound to the SAME `MockMosaicService`/`RelayTicker`/`CaptureComposer`/`StreamingVoiceRecorder` (full relay bring-up + scenePhase + voice wiring mirrored), forced `.graphite`. GrHeader, GrCaptureBar+GrCaptureSheet (reuse CaptureComposer + `MosaicService.capture`), GrTabPlaceholder. `#Preview` only — NOT the app entry until cutover (`TeselaApp` unchanged).
- **Gates (re-verified):** web `svelte-check` clean for graphite; iOS `xcodebuild` → BUILD SUCCEEDED. Old UI untouched on both.
- **Deferred check:** visual+interaction QA of `/g` (⌘K palette, Space leader) — user's Chrome held the MCP profile. Behavior is reused/mirrored (low risk); confirm at `localhost:5173/g`.
- **⚠ Cutover note:** the web palette/leader/commands reuse behavior modules under `lib/v4/` (`buildV4Commands`) + `lib/v5/` (`leader-tree`) + `lib/stores/`. At cutover, deleting the v4/v5 *UI routes/components* must NOT delete these reused *behavior* modules — separate them (move the reused logic out of the to-delete tree) first.

### NEXT — Daily-driver views (separate plan to author)
Fill the pane/tab bodies: daily journal, page/outliner (REUSE the CodeMirror `BlockOutliner` engine), inbox triage, agenda week, search. Web in the GrPane body; iOS in the GrTabPlaceholder slots. Then parity check → cutover (delete old v4/v5 web + old iOS Views; switch `TeselaApp` entry to GrAppShell; preserve reused behavior modules). Reference `gr-core.jsx`/`gr-pages.jsx` (web views) + `grm-*.jsx` (mobile views).
