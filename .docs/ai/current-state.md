# Current State
Branch: fix/desktop-wkwebview-drag

## Plan
- [x] Reproduce post-build-79 desktop failure boundary. Verify: Safari/WebKit + signed Tauri probe
- [x] Add dual-format drag locator and prevent failed-start session latch. Verify: focused RED/GREEN E2E
- [x] Run relocation and web gates. Verify: 13 E2E + 976 unit + `pnpm --dir web check`
- [x] Build and Apple Development-sign worktree bundle. Verify: `codesign --verify --deep --strict`
- [ ] Merge, install `/Applications`, and verify live health endpoint. Verify: `scripts/install-desktop.sh`

## Blockers
- Human: physical desktop drag after corrected bundle is installed.

## Open questions
- Taylor confirmed TestFlight build 79 Move to works; desktop remains the only product-test gate.
