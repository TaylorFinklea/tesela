# Cross-Platform Changelog Screen Report

## Outcome

- `release-notes/releases.json`: canonical schema-1, latest-first catalog; web/desktop/iOS current pointers.
- Web/Tauri: one-time auto-open; Settings About + `whats-new`; New/Fixed/Important; older history/detail/back; fail-soft fallback; focus restore.
- iOS: shared presenter across Graphite/legacy shells; post-onboarding auto-open; Settings + command; native history/detail; unavailable fallback.
- Release paths: exact desktop/iOS artifact validation; deterministic Markdown/plain render; JSON-safe updater notes; CI + copy-drift gates.

## Automated Verification

- `node --test scripts/tests/changelog.test.mjs`: 21 pass.
- `pnpm --dir web check:changelog`: valid 6-release catalog; iOS copy exact.
- `pnpm --dir web check:manifest` + `scripts/check-command-manifest-drift.sh`: exact/fresh.
- `pnpm --dir web check`: 0 errors; 48 pre-existing warnings.
- `pnpm --dir web test:unit`: 996 pass.
- `pnpm --dir web test:e2e -- release-notes.spec.ts`: 3 pass; auto, Settings/history, command/Esc.
- `pnpm --dir web build`: pass.
- `cargo test -p tesela-desktop`: 5 pass.
- `scripts/desktop-release.sh --skip-notarize` with disposable paths: catalog/version validated; Markdown/plain notes rendered; safe no-app exit.
- Full post-change iOS scheme: 581 pass, including 19 release-notes/command tests.
- `bash -n scripts/ios-testflight.sh` + exact `ios 1.1 (80)` validation: pass.

## Product QA

- iPhone 17 Simulator, built app: current `Tesela 1.1 (80)` sheet rendered with all three sections and Moshi-style latest-first layout.
- App preferences plist: `releaseNotes.lastSeen.ios = 2026-07-15.ios-1.1-80` only after render.
- Simulator relaunch: Today visible; current sheet did not reopen.
- Web Chromium E2E: manual Settings replay, newest-first older detail/back, Done, command replay, Esc, and automatic suppression.
- Desktop debug app bundle built at `target/debug/bundle/macos/Tesela.app`; Info.plist `0.1.2`.

## Residual Human Gate

- Installed desktop UI replay: not run because `/Applications/Tesela.app` was already active and Tauri single-instance redirected the disposable build. Preserve the daily-driver process; verify Settings/command/desktop label after installing the new bundle.

## Expected Existing Noise

- Svelte warnings unchanged; deprecated Loro container warnings unchanged.
- Simulator APNs entitlement warning expected; Swift concurrency warnings remain in existing transcription code.
