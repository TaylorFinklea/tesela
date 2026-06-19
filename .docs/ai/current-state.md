# Current State

## Branch
- `main` @ `ec1e2019` == origin/main — **PUSHED 2026-06-18** (everything below). Clean tree; `.docs/ai/review/` is untracked (the 3 open-source arch-review reports, kept for reference).

## Plan
- [x] **Arch-review eval + hygiene batch.** Adversarially verified the 3 open-source review reports (`.docs/ai/review/`) vs real code (ultracode, Claude-only) — ~20% signal, ~80% cargo-culted team/SaaS advice. Acted on the real findings: C23 (backup in-place-restore 409 guard — only data-loss item) + hygiene C19/C20/C21/C24/C6 (CI clippy + svelte-check blocking, `cargo audit`, delete `tesela-loro-spike`, fix `AGENTS.md` Next.js→Svelte + crate count, drop drifted `default_types()`). Declined the rest. See git log `3fec1b62`/`9d5d9b7c`.
- [x] **iOS editor sprint → TestFlight builds 21–28** (all pushed + Opus-verified xcodebuild+tests). Marker unification (Agenda/Inbox), Enter-indent + empty-outdent + insert-after-cursor, word-wrap (real fix = `sizeThatFits`), capture target-swatch menu, `[[`/`#`/`/` inline autocomplete on ONE trigger-detection framework (`EditorAutocomplete`/`LinkSuggest.detectTrigger`; `LinkSuggestTests` 18/18), complete page+tag source via new FFI `SyncEngineHandle.index_entries()` (Loro index, fixes unmaterialized-note gap), Graphite Search view. See git log + `decisions.md` 2026-06-18.
- [x] **#64 mobile command palette** (build 29) — keyboard-toolbar "Commands" button (new `.commandPalette` item) → `GrCommandPalette` sheet over the `GrCommand` catalog (tab nav + Sync now + Settings), via a `\.openCommandPalette` env action → `GrAppShell.runCommand`. Insert verbs stay in `/` slash; block actions stay on the toolbar. `GrCommandTests` 5/5.
- [x] **#65 capture sheet footer clipped behind keyboard** (build 30) — text-path autofocus was racing the sheet present-transition; deferred to 320ms so the keyboard rises against a settled layout. Intermittent keyboard-timing → needs on-device confirm.
- **iOS editor track COMPLETE** — every reported item shipped (TestFlight builds 21–30, all pushed).

## Blockers
- None.

## Open Questions
- Taylor verifying builds 28–30 on device (`#`/`/` autocomplete, command palette, capture-clip fix).
- **Next track is Taylor's pick** (no iOS items left): M3 sync spine (CF Worker deploy needs Taylor's CF acct) · Properties/types system (step 3b) · or more editor/daily-driver polish.
