# iOS foundation plan

Status: drafted 2026-05-13. Scope: how the iPhone app embeds the existing
Rust core and reaches feature parity with the desktop sync POC, then
extends with touch-first product features. Tracks the locked sync work
order: block-level → LAN/crypto → desktop product UX → **iOS (you are
here)**.

## Goal

A native SwiftUI iPhone app that:

1. Runs the same `tesela-sync` engine the desktop runs, embedded via
   UniFFI — no second implementation of the merge logic.
2. Pairs with a desktop instance using the same Phase 2.2 pairing code,
   over the same Phase 2.3 AEAD wire.
3. Edits notes touch-first (no vim, no leader keys) and syncs them back
   to the desktop the same way two desktops sync today.

Non-goals for this phase:

- iPad layout (revisit after iPhone ships; see `project_mobile_strategy`).
- Voice power menu (Phase 3 per the long-term plan).
- A second-system rewrite of features that already work on desktop.
  Mobile feature parity follows desktop, not the other way around.

## Decisions in effect

Carried from earlier memory entries; restated here so this doc reads
self-contained.

- **Native, not PWA.** SwiftUI, separate codebase.
- **Rust core via UniFFI**, not rewritten in Swift.
- **Sync wire**: the desktop's `_tesela._tcp.local.` + AEAD-sealed
  envelopes are the protocol. The iPhone advertises and discovers on
  the same mDNS service type; it speaks the same `/sync/peer/*` HTTP
  shape.
- **Foreground-only sync on iPhone.** No background daemon. App-launch
  trigger + a polling timer while foreground + a flush on app close.
  APNs proxy is a future, not-blocking refinement.
- **No central directory** (no relay, no account). Pairing is
  out-of-band: a QR code carrying the same Slice 2 base64url payload.

## Milestones

Each milestone ends with a concrete check the user can run.

### M1 — UniFFI bridge crate

Create `crates/tesela-sync-ffi`. Wraps `tesela-sync` with a UniFFI-
exported surface. Start small: a hello-world (`tesela_sync_version`),
then `generate_device_id` and the pairing-code encode/decode. The
existing public types in `tesela-sync` were written FFI-clean from
day one (no borrows, no generics, owned errors), so this layer is a
mechanical wrap, not a refactor.

**Check:** `cargo build -p tesela-sync-ffi` succeeds; running
`uniffi-bindgen generate` produces a `.swift` file that parses
without errors.

### M2 — iOS cross-compile

`crate-type = ["staticlib", "cdylib", "lib"]` is already set on the FFI
crate. What's still needed:

1. **Switch Rust toolchain manager.** The repo currently uses Homebrew's
   `rust` formula, which ships only the host stdlib and doesn't support
   per-target `std` like iOS. The fix is a one-time install of rustup
   alongside (or replacing) brew rust:

   ```
   brew uninstall rust            # optional; rustup ships rustc/cargo too
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup toolchain install stable
   ```

2. **Add iOS targets:**

   ```
   rustup target add aarch64-apple-ios          # iPhone, real device
   rustup target add aarch64-apple-ios-sim      # iPhone simulator on Apple Silicon
   rustup target add x86_64-apple-ios           # simulator on Intel Macs (optional)
   ```

3. **Cross-compile and verify:**

   ```
   cargo build -p tesela-sync-ffi --target aarch64-apple-ios-sim --release
   ls target/aarch64-apple-ios-sim/release/libtesela_sync_ffi.a
   ```

This whole milestone is mechanical once rustup is in place; the M1 FFI
crate is already shaped for it (`staticlib` artifact + no host-only
deps).

**Check:** `libtesela_sync_ffi.a` exists at the path above.

### M3 — Xcode project scaffold

New `app/Tesela-iOS/` (sibling to a future `app/Tesela-macOS/` if we
revive the frozen macOS app). Minimum app: a single SwiftUI view that
calls `tesela_sync_version()` through the bridge and renders the
result. Confirms the .a + .swift + Bridging-Header glue is wired.

**Check:** App launches in the iPhone simulator and shows the version
string.

### M4 — Sync UI parity

Bring the desktop's Settings → Devices flow over: device id card,
LAN discovered list, paired list, pair-via-code paste, "Sync now."
The view is touch-first (large hit targets, native pickers, no
hover affordances) but the data and endpoints are the desktop's
`/sync/peer/*` set unchanged.

iOS-specific bit: mDNS goes through `NSNetService` (or the
`Network.framework` `NWBrowser`) rather than `mdns-sd`, because the
iOS sandbox / entitlements path is different. The Rust core stays
Bluetooth-blind for now; mDNS is a Swift layer that hands resolved
peers down to the FFI as `(device_id_hex, host, port)` triples.

**Check:** iPhone simulator on the same LAN as a running
`tesela-server`: opens "Devices," sees the desktop in the LAN
discovered list, pastes the desktop's pairing code, syncs a test
note round-trip. Same gestures as the desktop POC.

### M5 — Touch-first outliner

The outline view from the web client, ported. Drop vim. Add:

- Tap a block to focus, again to enter edit mode.
- Swipe-right / -left on a block to indent / outdent.
- Long-press a block for a context power menu.
- Drag-and-drop reorder.
- The bid HTML comment stays hidden, same trick as the web client.

**Check:** Edit a block on iPhone; the change shows on desktop within
one daemon tick. Concurrent block edits converge.

### M6 — Daily note + quick capture

Front door is today's daily note. A persistent floating "+" composes
a new bullet that lands in the daily as the bottom block. Share-sheet
integration → append to today's daily. Shortcuts intent for "Add
Tesela note." iOS keyboard quick bar: bullet, indent, outdent, tag.

**Check:** Capture a thought via the share sheet from Safari; see
it on desktop next sync tick. Tap the floating + during a meeting,
type three blocks, leave the app, return — they're persisted and
syncing.

### M7 — Run on real device

Provisioning, code signing, install on an actual iPhone. Confirm
sync over the home LAN. Document any iOS-specific gotchas (background
suspend timing, network entitlement prompts, low-power mode pauses
on the polling timer).

**Check:** Pair the real iPhone with the desktop on the home LAN.
Edit a note on each, confirm both sides converge after each comes
back to foreground.

## Order of execution

M1 → M2 → M3 → M4 happen in sequence (each gates the next). M5 and
M6 can interleave once M4 lands — the engine + sync wire are stable
by then. M7 is the integration sit-down at the end.

While I'm offline from the user's machine, M1 + M2 are the natural
chunks: pure Rust, fully testable, no device needed.

## Threats / open questions

- **mDNS on iOS.** `mdns-sd` (the crate the desktop uses) doesn't run
  on iOS without entitlements gymnastics. M4 will use the platform
  framework instead and pass discovered peers down to Rust as plain
  data. Cleaner.
- **Background sync.** iPhone's foreground-only constraint is real.
  If "I edited on laptop, where is it on phone" lag is annoying,
  next refinement is a stateless APNs proxy that nudges the phone to
  wake and pull. Not in this phase.
- **Keychain for group key.** Today the group key lives in
  `<mosaic>/.tesela/group_key.bin`. iOS Keychain is the proper home;
  the storage adapter trait in `crypto/keys.rs` is already shaped for
  this swap.
- **UniFFI async.** The engine is async (Tokio). UniFFI ≥ 0.25
  supports async exports, but bringing Tokio into an iOS process has
  some footprint cost. Acceptable; documented for if it becomes a
  power-management issue.
- **Update channel.** Notify the iPhone to refresh after a successful
  apply (WebSocket from the desktop, push, or polling). Polling is
  the simplest first cut and fits the foreground-only sync rhythm.

## Eventual: macOS native app

The macOS SwiftUI app described in `project_gui_vision` is currently
frozen. It can revive as a Mac Catalyst target of this iOS app once
it stabilizes — same Rust core via the same FFI crate, just a
different SwiftUI surface. Not in scope for this phase, but the
architecture is set up for it.
