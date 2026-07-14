# Current State
Branch: main

## Plan
- [x] Reproduce desktop dragstart cancellation/session latch. Verify: Safari/WebKit + signed Tauri probe
- [x] Add dual-format move locator and session-match guards. Verify: focused RED/GREEN E2E
- [x] Run relocation/web gates. Verify: 13 E2E + 976 unit + `pnpm --dir web check`
- [x] Merge, rebuild, sign, install, and relaunch. Verify: `scripts/install-desktop.sh`
- [?] Taylor physically drags a parent plus children between days and verifies persistence after relaunch.

## Blockers
- Human: physical desktop drag in installed `/Applications/Tesela.app`.

## Open questions
- None; TestFlight build 79 Move to already passed physical iOS QA.
