# iOS home-screen widgets — report

## Delivered

- bead `tesela-a4m.1`
- distinct Tesela Agenda and Tesela Inbox WidgetKit gallery entries
- small + medium layouts; bounded read-only rows, honest empty/missing states,
  relative snapshot age, privacy-sensitive content
- atomic version-1 JSON snapshot in App Group `group.app.tesela.shared`
- main app publishes the canonical seven-day Agenda and built-in Inbox query;
  extension performs no query, sync, Keychain, FFI, or network work
- whole-tile navigation: Agenda → Agenda tab, Inbox → Views tab, in Graphite
  and legacy shells
- TestFlight build-number stamping covers both app and widget plists

## Product proof

- iPhone 17 Simulator, iOS 26.5: system widget gallery exposed all four
  configurations (Agenda/Inbox × small/medium)
- real small Agenda and Inbox tiles added to SpringBoard and rendered from the
  App Group snapshot: “No open tasks in the next seven days” and “Inbox is
  clear” for the mock mosaic
- actual Inbox tile tap selected Views; actual Agenda tile tap selected Agenda
- installed simulator bundle contains `PlugIns/TeselaWidgets.appex`; simulator
  App Group container exists and holds a version-1 snapshot
- no user QA remains for this slice

## Automated evidence

- `xcodegen generate --spec app/Tesela-iOS/project.yml` — pass
- full iOS scheme: 591 unit tests + 2 XCUITests, zero failures
- generic iOS Simulator build — pass; embedded extension validation and host +
  extension App Group entitlements confirmed in build output
- focused snapshot tests cover codec/version rejection, atomic store round-trip,
  projection limits, mock publication, App Group availability, and route parsing
- app/widget plists + entitlements lint — pass
- TestFlight script syntax — pass
- `git diff --check` — pass

## Release boundary

- generic device build reaches provisioning and fails because Apple has no
  development profile for `app.tesela.ios.widgets`, while the existing
  `app.tesela.ios` profile predates App Groups and lacks
  `group.app.tesela.shared`
- the managed App Store Connect credential could not refresh profiles because
  the BWS bootstrap token remained unavailable in the login Keychain after the
  admin restart
- follow-up `tesela-a4m.2` owns App Group registration/profile refresh and an
  exact generic-device signing gate; it is release operations, not product QA

## Known baseline

- `cargo fmt --all -- --check` still reports unrelated pre-existing Rust
  formatting drift; this slice changes no Rust source
