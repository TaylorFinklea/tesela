# Current State

## Branch
- `main` @ `704592f4` == origin/main — **PUSHED 2026-06-18** (everything below). Clean tree; `.docs/ai/review/` is untracked (the 3 open-source arch-review reports, kept for reference).

## Plan
- [x] **Arch-review eval + hygiene batch.** Adversarially verified the 3 open-source review reports (`.docs/ai/review/`) vs real code (ultracode, Claude-only) — ~20% signal, ~80% cargo-culted team/SaaS advice. Acted on the real findings: C23 (backup in-place-restore 409 guard — only data-loss item) + hygiene C19/C20/C21/C24/C6 (CI clippy + svelte-check blocking, `cargo audit`, delete `tesela-loro-spike`, fix `AGENTS.md` Next.js→Svelte + crate count, drop drifted `default_types()`). Declined the rest. See git log `3fec1b62`/`9d5d9b7c`.
- [x] **iOS editor sprint → TestFlight builds 21–28** (all pushed + Opus-verified xcodebuild+tests). Marker unification (Agenda/Inbox), Enter-indent + empty-outdent + insert-after-cursor, word-wrap (real fix = `sizeThatFits`), capture target-swatch menu, `[[`/`#`/`/` inline autocomplete on ONE trigger-detection framework (`EditorAutocomplete`/`LinkSuggest.detectTrigger`; `LinkSuggestTests` 18/18), complete page+tag source via new FFI `SyncEngineHandle.index_entries()` (Loro index, fixes unmaterialized-note gap), Graphite Search view. See git log + `decisions.md` 2026-06-18.
- [x] **#64 mobile command palette** (build 29) — keyboard-toolbar "Commands" button (new `.commandPalette` item) → `GrCommandPalette` sheet over the `GrCommand` catalog (tab nav + Sync now + Settings), via a `\.openCommandPalette` env action → `GrAppShell.runCommand`. Insert verbs stay in `/` slash; block actions stay on the toolbar. `GrCommandTests` 5/5.
- [x] **#65 capture sheet footer clipped behind keyboard** (build 30) — text-path autofocus was racing the sheet present-transition; deferred to 320ms so the keyboard rises against a settled layout. Intermittent keyboard-timing → needs on-device confirm.
- **iOS editor track COMPLETE** — every reported item shipped (TestFlight builds 21–30, all pushed).
- [x] **Cross-device sync bug (#70) + durability P1** (2026-06-18). (a) Desktop /g didn't live-update on relay-pulled remote edits — `sync_relay::TickOutcome.applied_updates` now carries the applied bytes + the daemon re-broadcasts them (the post-apply re-export returned None). **DESKTOP-ONLY; deploy pending** (Tauri rebuild + /Applications swap — task #73; `web/build` already rebuilt). (b) ⚠ The scary one: an iOS capture sat unpushed to the relay for 2h (foreground-only push; background stranded the queue). **P1 shipped (build 31):** `RelayTicker.flushOnBackground()` drains the outbound queue in a `UIApplication` bg task before suspend. Taylor chose **"go big"** → full plan in `phases/2026-06-18-sync-durability-spec.md`.
- [ ] **Sync durability P2** (#71): BGProcessingTask catch-up + background URLSession. **P3** (#72): APNs silent-push (instant cross-device; needs Taylor's APNs key + relay/CF work).

## Blockers
- None (sync durability P2/P3 are planned phases, not blockers).

## Open Questions
- Taylor verifying builds 28–31 on device (autocomplete, palette, capture-clip, **flush-on-background**: capture → immediately background → block reaches other devices without relaunching the phone).
- **#70 desktop deploy** (#73): rebuild the Tauri app + swap /Applications — Taylor's running-app environment, needs his OK/involvement.
- Sync durability: continue P2 → P3 (the rock-solid endgame) vs interleave other tracks.
