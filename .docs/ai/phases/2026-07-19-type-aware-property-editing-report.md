# Type-aware property editing report — tesela-11v

## Outcome

- web: checkbox buttons, multi-select Save/Cancel checklist, safe URL/email/phone links
- server: member-delta route; markdown-only notes seed into Loro before first typed property write
- sync/FFI: canonical typed scalars; mergeable list member operations; concurrent first-touch union
- iOS: native Link/Button chips, item-driven edit sheet, HTTP + relay write wiring
- iOS layout: property chips use FlowLayout; no narrow-screen vertical-letter collapse

## Product proof

- fresh markdown-only HTTP fixture on iPhone 17 simulator
- checkbox false → true; persisted through process kill/relaunch
- multi-select gamma + Cancel: no write; beta → gamma + Save: `+[gamma] -[beta]`
- URL draft Cancel preserved value; Save persisted edit; link opened Safari
- final materialized state: pinned true, labels alpha/gamma, edited URL
- hdeck: `20260719-type-aware-property-editing` (done, no open ask)
- TestFlight 81 product ask remains `20260719-testflight-81-widgets-product-test`

## Verification

- `cargo test -p tesela-sync`: 300 passed, 1 ignored; integration suites passed
- `cargo test -p tesela-sync-ffi`: 49 passed
- `cargo test -p tesela-server --test set_property_engine`: 6 passed
- `pnpm --dir web test:unit`: 1015 passed
- `pnpm --dir web check`: 0 errors, 45 pre-existing warnings in 17 files
- focused Xcode simulator tests: 18 passed
- simulator build/run + kill/relaunch: passed
- `bash scripts/check-ffi-drift.sh`: 5 generated files in sync
- `git diff --check` + focused Rust formatting: passed

## Verification boundary

- ordered repo gate stopped at `cargo fmt --all -- --check`: existing workspace-wide Rust formatting drift outside this feature
- unrelated formatter-only changes in `routes/commands.rs` and `routes/views.rs` left out of the feature commit
- feature is not included in TestFlight build 81; next release product checklist is in hdeck
