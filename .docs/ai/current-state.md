# Current State

## Branch
- `main` @ HEAD — **2026-06-19**. Clean tree; `.docs/ai/review/` is untracked (the 3 open-source arch-review reports, kept for reference).

## This session
- [x] **iOS: past dailies no longer dimmed** (build 34 → TestFlight). Dropped `.opacity(0.72/0.6)` on yesterday/past-daily blocks in `GrDailyView.swift` (live Graphite shell) — Taylor disliked the grayed/darker look. Legacy `DailyView`/`WorkspaceGridView` still dim but aren't live (`useLegacyShell` off); offered to strip those too.

## Plan
- [x] **Arch-review eval + hygiene batch.** Adversarially verified the 3 open-source review reports (`.docs/ai/review/`) vs real code (ultracode, Claude-only) — ~20% signal, ~80% cargo-culted team/SaaS advice. Acted on the real findings: C23 (backup in-place-restore 409 guard — only data-loss item) + hygiene C19/C20/C21/C24/C6 (CI clippy + svelte-check blocking, `cargo audit`, delete `tesela-loro-spike`, fix `AGENTS.md` Next.js→Svelte + crate count, drop drifted `default_types()`). Declined the rest. See git log `3fec1b62`/`9d5d9b7c`.
- [x] **iOS editor sprint → TestFlight builds 21–28** (all pushed + Opus-verified xcodebuild+tests). Marker unification (Agenda/Inbox), Enter-indent + empty-outdent + insert-after-cursor, word-wrap (real fix = `sizeThatFits`), capture target-swatch menu, `[[`/`#`/`/` inline autocomplete on ONE trigger-detection framework (`EditorAutocomplete`/`LinkSuggest.detectTrigger`; `LinkSuggestTests` 18/18), complete page+tag source via new FFI `SyncEngineHandle.index_entries()` (Loro index, fixes unmaterialized-note gap), Graphite Search view. See git log + `decisions.md` 2026-06-18.
- [x] **#64 mobile command palette** (build 29) — keyboard-toolbar "Commands" button (new `.commandPalette` item) → `GrCommandPalette` sheet over the `GrCommand` catalog (tab nav + Sync now + Settings), via a `\.openCommandPalette` env action → `GrAppShell.runCommand`. Insert verbs stay in `/` slash; block actions stay on the toolbar. `GrCommandTests` 5/5.
- [x] **#65 capture sheet footer clipped behind keyboard** (build 30) — text-path autofocus was racing the sheet present-transition; deferred to 320ms so the keyboard rises against a settled layout. Intermittent keyboard-timing → needs on-device confirm.
- **iOS editor track COMPLETE** — every reported item shipped (TestFlight builds 21–30, all pushed).
- [x] **Cross-device sync bug (#70) + durability P1** (2026-06-18). (a) Desktop /g didn't live-update on relay-pulled remote edits — `sync_relay::TickOutcome.applied_updates` now carries the applied bytes + the daemon re-broadcasts them (the post-apply re-export returned None). **DESKTOP-ONLY; deploy pending** (Tauri rebuild + /Applications swap — task #73; `web/build` already rebuilt). (b) ⚠ The scary one: an iOS capture sat unpushed to the relay for 2h (foreground-only push; background stranded the queue). **P1 shipped (build 31):** `RelayTicker.flushOnBackground()` drains the outbound queue in a `UIApplication` bg task before suspend. Taylor chose **"go big"** → full plan in `phases/2026-06-18-sync-durability-spec.md`.
- [x] **Sync durability P2a** (#71, build 32): BGProcessingTask catch-up (`app.tesela.ios.relay-catchup` → `RelayTicker.runBackgroundCatchup()`). Built via a **3-way pi head-to-head** (minimax-m3 won vs gpt-5.5 + qwen3.7-max; worktree-isolated; all 3 build-verified + scored).
- [x] **P2b = DEAD END** (#71): the relay PUT is Rust/`reqwest` (`transport/relay.rs:248`) called via FFI, NOT Swift `URLSession` — a background URLSession can't carry it, and it's redundant with P1 (~30s bg task) + P2a (periodic retry). Closed; no code.
- [x] **Sync durability P3a** (#72, build TBD this session): iOS APNs silent-push receiver — `AppDelegate` (via `@UIApplicationDelegateAdaptor`) registers for remote notifications, captures the token, and on a content-available push runs `runBackgroundCatchup()`. `UIBackgroundModes += remote-notification`. Built via a **4-way pi head-to-head** (minimax won vs gpt-5.5/qwen/kimi; ALL 4 build-verified + scored). **Not functional end-to-end** — see P3b/P3c + Taylor deps.
- [ ] **Sync durability P3b/P3c** (#72): relay device-token registry + relay APNs send (HA add-on AND CF Worker). **⚠ TAYLOR DEPS:** (1) enable Push for App ID `app.tesela.ios` + `aps-environment` entitlement, (2) APNs auth key (`.p8`+ids). Detail in `phases/2026-06-18-sync-durability-spec.md`.

## Blockers
- None (sync durability P2/P3 are planned phases, not blockers).

## Open Questions
- Taylor verifying builds 28–31 on device (autocomplete, palette, capture-clip, **flush-on-background**: capture → immediately background → block reaches other devices without relaunching the phone).
- **#70 desktop deploy** (#73): rebuild the Tauri app + swap /Applications — Taylor's running-app environment, needs his OK/involvement.
- Sync durability: continue P2 → P3 (the rock-solid endgame) vs interleave other tracks.
