# Changelog Screen Spec

Status: approved design; implementation plan pending written-spec review
Bead: `tesela-8h0`
Decision: `decisions.md` — 2026-07-15 changelog entry

## Objective

- User-facing “What’s New” experience across iOS, hosted web, and Tauri desktop.
- Auto-present the current release once per platform/device.
- Latest-first detail; older applicable releases one action away.
- Offline and non-blocking.
- Curated release copy: New / Fixed / Important.

## Scope

- One canonical bundled manifest.
- Web/Tauri shared Svelte presentation.
- Native SwiftUI presentation.
- Settings + shared command-registry entry points.
- Per-platform seen state.
- Release-time validation and rendering.
- Initial current entry plus at least two verified older product releases per platform.

## Non-goals

- No runtime GitHub/App Store fetch.
- No Markdown/HTML authoring or arbitrary rich content.
- No release-note editor UI.
- No migration of historical `RELEASE.md`.
- No redesign of Settings information architecture.
- No change to generic CLI release versioning.

## Canonical data contract

Path: `release-notes/releases.json`.

```json
{
  "schemaVersion": 1,
  "current": {
    "web": "2026-07-15.1",
    "desktop": "2026-07-15.1",
    "ios": "2026-07-15.1"
  },
  "releases": [
    {
      "id": "2026-07-15.1",
      "publishedAt": "2026-07-15T12:00:00Z",
      "title": "Sharper daily work",
      "summary": "A focused summary in plain language.",
      "platforms": ["web", "desktop", "ios"],
      "versions": {
        "desktop": "0.1.2",
        "ios": { "marketing": "1.1", "build": "80" }
      },
      "new": ["One concrete user-visible addition."],
      "fixed": ["One concrete user-visible repair."],
      "important": ["One action or behavior users must know about."]
    }
  ]
}
```

Rules:

- `schemaVersion`: exact supported integer.
- `current`: required pointer per platform; pointer target must include that platform.
- `releases`: globally newest-first by `publishedAt`; stable unique IDs.
- `publishedAt`: RFC 3339 UTC.
- `platforms`: unique subset of `web`, `desktop`, `ios`.
- Desktop release: `versions.desktop` required and exact.
- iOS release: marketing/build pair required and exact.
- Web release identity: `current.web`; no separate binary version.
- Section arrays always present; individual arrays may be empty; total item count must be positive.
- Plain strings only; renderer owns bullets, colors, icons, and emphasis.
- Platform history: filter to platform, locate its current pointer, return current plus older entries only. Never expose a newer unreleased entry for that platform.

Bundling:

- Canonical JSON stays single-source.
- Web build consumes canonical bytes directly or a byte-identical generated artifact checked against canonical.
- Xcode target bundles canonical bytes directly or a byte-identical resource checked against canonical.
- No client-specific hand-maintained copy.

## Presentation state

Per-platform persisted key:

- Web/Tauri: `tesela:releaseNotes:lastSeen:<platform>` in local storage.
- iOS: `releaseNotes.lastSeen.ios` in app storage.

Auto-presentation algorithm over the full platform-filtered newest-first list:

1. Resolve current pointer and current index.
2. Missing/unknown last-seen ID: present current.
3. Last-seen index greater than current index: present current; installed release advanced.
4. Last-seen index equal to or less than current index: do not present; current already seen or app downgraded.
5. Persist current ID only after valid current detail actually renders.
6. Browsing an older entry never changes last-seen state.

First install:

- Onboarding owns first-run priority.
- Defer auto-presentation until the first usable shell appears.
- Missing last-seen then presents current once; manual replay remains available.

## Selected interaction: latest first

Current detail:

- Header: “What’s New”; platform version/build + release date.
- Title + one-sentence summary.
- New / Fixed / Important groups; omit empty groups.
- Important group uses distinct warning treatment without blocking dismissal.
- “View older releases” only when older applicable entries exist; include count.
- Done/close action always available.

History:

- Compact newest-first list excluding the already-visible current entry.
- Row: platform version where available, date, title, summary.
- Selecting a row reuses the detail renderer.
- Back returns to history; close exits the whole experience.

Accessibility and keyboard:

- Semantic headings and lists.
- Focus moves into the surface on open and returns to the invoking control on close.
- Web/Tauri: Esc closes; Enter/Space activates; list supports normal tab order and arrow-key navigation where the existing overlay pattern supports it.
- iOS: native NavigationStack, Dynamic Type, VoiceOver labels, Done, back navigation, and swipe dismissal.

## Web and Tauri

Shared modules:

- Manifest types/parser and platform-history selector: pure TypeScript.
- Seen-state decision: pure function with injected storage adapter.
- `ReleaseNotesOverlay.svelte`: current/history/detail states.
- Fullscreen overlay store gains a release-notes kind; existing Esc/focus conventions remain authoritative.

Platform resolution:

- Hosted browser defaults to `web`.
- Tauri initialization declares `desktop`; no user-agent inference.
- Tauri auto-updater restarts into the new bundle; bundled pointer + seen state trigger the post-update surface.

Entry points:

- Automatic shell presentation after mount/hydration.
- Settings General/About row: dynamic platform version and “What’s New”.
- Shared command ID `whats-new`; available to palette and appropriate global dispatchers.
- Tauri native menu remains unchanged; command registry is the product action surface.

## iOS

Shared native modules:

- `Decodable` catalog matching schema version 1.
- Pure platform-history and should-present logic.
- `ReleaseNotesView`: NavigationStack owning current/history/detail.
- One shell-level presenter reused by default Graphite and legacy escape-hatch shells; no duplicated seen logic.

Entry points:

- Automatic presentation after onboarding and shell readiness.
- Graphite Settings About card: dynamic Bundle version/build + “What’s New”.
- Legacy Settings footer becomes the same dynamic About entry.
- Shared command manifest entry `whats-new`; native executable-ID map and shell dispatcher open the sheet.

Dismissal:

- Done or interactive sheet dismissal.
- Presentation counts as seen once valid current detail appears.
- Changelog failure never interferes with onboarding, mosaic activation, sync, or editing.

## Release tooling

New deterministic tool: `scripts/changelog.mjs`.

Commands:

- `validate`: full schema/order/pointer/platform validation.
- `validate --platform web|desktop|ios [--version V] [--build N]`: current-pointer and artifact-version validation.
- `render --release ID --format markdown|plain`: user-facing release copy from the manifest.

Integration:

- CI/web build: full validation + web pointer validation.
- `scripts/desktop-release.sh`: validate current desktop version before build; render curated Markdown for GitHub and safely serialize plain notes into `latest.json`.
- `scripts/ios-testflight.sh`: after build-number stamp and before archive, validate marketing/build pair; render a plain-text operator artifact for App Store/TestFlight copy.
- Generic GitHub CLI workflows: validate canonical data in CI; do not advance client pointers or make `RELEASE.md` the runtime source.
- Release aborts before publishing when validation fails.

Validation failures:

- Unknown schema.
- Missing/extra malformed top-level fields.
- Duplicate release IDs.
- Non-descending or invalid timestamps.
- Invalid/duplicate platform values.
- Missing current pointer or target not applicable to platform.
- Missing desktop/iOS version metadata.
- Artifact-version mismatch.
- Empty title/summary/items or release with no change items.

## Runtime failure semantics

- Missing/malformed manifest: log once; skip automatic presentation.
- Invalid current pointer: log once; skip automatic presentation.
- Manual entry with unavailable catalog: quiet “Release notes unavailable” state + close action.
- Storage read failure: treat as unknown; allow current display.
- Storage write failure: keep a session-memory seen value to prevent a loop during the running session; never crash.
- Unknown future fields: decoder may ignore; unknown schema version still rejects.
- No network dependency and no retry spinner.

## Automated verification

Manifest/tool:

- Valid catalog and rendered Markdown/plain snapshots.
- Every validation failure class above.
- Safe escaping of quotes, Unicode, and newlines in updater JSON.

Web:

- Parser, platform history, current slicing, downgrade, unknown last-seen, and storage-failure unit tests.
- Command/Settings entry points open current detail.
- Auto-open once; reload does not reopen.
- Older list/detail/back/close.
- Empty sections, no-history, unavailable state, Esc, and focus restoration.
- `pnpm --dir web check`.
- `pnpm --dir web test:unit`.
- Relevant `pnpm --dir web test:e2e` coverage.

iOS:

- Decoder and all selector/seen-state branches.
- Current/history/detail navigation.
- Dynamic Bundle version/build.
- Graphite + legacy presenter/Settings entry compilation.
- `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`.

Desktop:

- Desktop platform declaration reaches the webview.
- Updater manifest uses curated notes and valid JSON.
- `cargo test -p tesela-desktop`.

## Manual QA acceptance

- Fresh current release appears once after usable shell/onboarding.
- Close via Done, Esc, and iOS swipe; normal app use remains available.
- Relaunch does not reopen the same release.
- Advancing current pointer opens exactly once.
- Downgrading does not reopen an older already-passed release.
- Settings and command palette reopen current release.
- Older list shows only released/applicable entries; selection/back work.
- New / Fixed / Important render correctly; empty groups disappear.
- Malformed manifest cannot block app launch.
- Hosted web, installed signed desktop bundle, and iOS Simulator/device each receive a product-level check.

## Completion

- All automated gates green.
- QA checklist delivered.
- `tesela-8h0` closed with verification evidence.
- Handoff state cleared or advanced to the next active item.
