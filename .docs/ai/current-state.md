# Current State
Branch: main

## Plan
- [x] Desktop shell receives HTML drag events. Verify: `cargo test -p tesela-desktop`
- [x] Durable subtree relocation through UniFFI. Verify: `cargo test -p tesela-sync-ffi`
- [x] iOS Move to UI/service, cancellation, and recovery. Verify: full simulator suite
- [x] Rebuild desktop, upload build 79, revise product-test report. Verify: installed signature + App Store upload success

## Blockers
- Human: click **Always Allow** for `/Applications/Tesela.app` Keychain access.

## Open questions
- Taylor device QA: desktop drag + TestFlight build 79 Move to.
