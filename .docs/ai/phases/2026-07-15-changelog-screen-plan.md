# Changelog Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan. This session executes inline because the active harness policy forbids unrequested subagents.

**Goal:** Ship a bundled, latest-first “What’s New” experience that opens once per release and remains available from Settings and the shared command palette on hosted web, Tauri desktop, and iOS.

**Architecture:** `release-notes/releases.json` is the canonical, curated catalog. A deterministic Node tool validates and renders it for CI and release automation. Svelte and SwiftUI each decode the same schema, apply the same platform-history and last-seen rules, and render native presentation surfaces; only iOS carries a byte-identical bundled copy because XcodeGen cannot reference the repo-root resource directly.

**Tech Stack:** JSON schema-by-code, Node 24 ESM + `node:test`, Svelte 5/TypeScript, Tauri 2/Rust, SwiftUI/XCTest, Bash release scripts, GitHub Actions.

## Global Constraints

- Follow `.docs/ai/phases/2026-07-15-changelog-screen-spec.md` exactly; its schema and seen-state rules are normative.
- Keep `release-notes/releases.json` canonical. `app/Tesela-iOS/Sources/Data/ReleaseNotes.json` must be byte-identical and guarded by a drift check.
- Runtime decode/storage failures fail soft and never delay onboarding, shell activation, sync, or editing.
- Do not touch the concurrent unstaged files under `crates/tesela-sync/src/engine/loro_engine/`.
- Use test-first checkpoints, but make one final scoped implementation commit for bead `tesela-8h0` rather than one commit per task.
- Do not push.

---

### Task 1: Canonical catalog, validation, and rendering

**Files:**

- Create: `release-notes/releases.json`
- Create: `scripts/changelog-lib.mjs`
- Create: `scripts/changelog.mjs`
- Create: `scripts/tests/changelog.test.mjs`

**Step 1: Write failing tool tests**

Cover these exact contracts in `scripts/tests/changelog.test.mjs`:

- valid schema-1 catalog passes and returns normalized release data;
- unknown schema, malformed top level, duplicate IDs, non-descending/invalid timestamps, invalid/duplicate platforms, missing/misdirected current pointers, missing desktop/iOS versions, blank strings/items, and zero total items fail with field-specific messages;
- platform version/build validation passes exact values and fails mismatches;
- Markdown and plain renderers omit empty groups and preserve quotes, Unicode, and embedded newlines safely;
- updater JSON built from rendered notes parses back to the original note text.

Run: `node --test scripts/tests/changelog.test.mjs`

Expected: FAIL because `scripts/changelog-lib.mjs` does not exist.

**Step 2: Implement the pure library**

Export these spec-derived interfaces from `scripts/changelog-lib.mjs`:

```js
export const PLATFORMS = ["web", "desktop", "ios"];
export function validateCatalog(input, artifact = {}) {}
export function platformHistory(catalog, platform) {}
export function renderRelease(release, format) {}
export function buildUpdaterManifest({ version, notes, pubDate, target, signature, url }) {}
```

`validateCatalog` returns the catalog on success and throws one aggregated `Error` on failure. Optional `artifact` accepts `{ platform, version, build }`. `platformHistory` filters to the selected platform and slices from its `current` pointer so unreleased newer entries never leak.

**Step 3: Implement the CLI**

`scripts/changelog.mjs` must support exactly:

```text
validate [--platform web|desktop|ios] [--version V] [--build N]
render --release ID --format markdown|plain
```

It reads `release-notes/releases.json` relative to its own module URL, writes rendered content only to stdout, writes validation errors to stderr, and exits nonzero on invalid arguments/catalog/version.

**Step 4: Seed verified release history**

Add globally newest-first entries:

- current web/desktop `2026-07-15.desktop-0.1.2`: What’s New surface plus physically verified Dailies subtree drag/multiline/nested-placement work; desktop version `0.1.2`;
- current iOS `2026-07-15.ios-1.1-80`: What’s New surface, build-79 subtree relocation, and current relay-durability fixes; iOS `1.1` build `80`;
- older web/desktop `2026-07-02.desktop-0.1.1`: command search/saved-view depth and signed auto-update support; desktop `0.1.1`;
- older iOS `2026-07-14.ios-1.1-79`: verified Move to subtree relocation; iOS `1.1` build `79`;
- older iOS `2026-07-08.ios-1.1-75`: live streaming dictation; iOS `1.1` build `75`;
- older web/desktop `2026-06-04.desktop-0.1.0`: Graphite daily workspace in the hosted and native shells; desktop `0.1.0`.

Copy only user-visible, verified claims from the named commits/roadmap evidence. Keep each section plain text and omit no required arrays.

**Step 5: Verify**

Run: `node --test scripts/tests/changelog.test.mjs`

Expected: PASS.

Run: `node scripts/changelog.mjs validate`

Expected: `release notes valid (6 releases)`.

---

### Task 2: Web catalog and seen-state domain

**Files:**

- Create: `web/src/lib/release-notes.ts`
- Create: `web/tests/unit/release-notes.test.mjs`

**Step 1: Write failing pure-domain tests**

Import the real TypeScript module from Node 24 and cover:

- strict schema-1 parse and fail-soft `loadBundledReleaseNotes` wrapper;
- hosted `web` default and injected Tauri `desktop` platform;
- platform filtering and current-plus-older slicing;
- missing/unknown last-seen => present;
- last-seen older than current => present;
- last-seen current or newer than current => do not present (seen/downgrade);
- storage read failure => present;
- storage write failure => session-memory suppression;
- browsing older releases never mutates seen state;
- web/desktop version labels.

Run: `node --test web/tests/unit/release-notes.test.mjs`

Expected: FAIL because the module does not exist.

**Step 2: Implement the domain module**

Export these interfaces from `web/src/lib/release-notes.ts`:

```ts
export type ReleasePlatform = "web" | "desktop" | "ios";
export interface ReleaseNote { /* exact schema-1 fields */ }
export interface ReleaseCatalog { /* exact schema-1 fields */ }
export interface SeenStorage { getItem(key: string): string | null; setItem(key: string, value: string): void }
export function parseReleaseCatalog(input: unknown): ReleaseCatalog;
export function loadBundledReleaseNotes(): ReleaseCatalog | null;
export function resolveReleasePlatform(host?: { __TESELA_PLATFORM__?: string }): "web" | "desktop";
export function platformReleaseHistory(catalog: ReleaseCatalog, platform: ReleasePlatform): ReleaseNote[];
export function shouldPresentCurrent(catalog: ReleaseCatalog, platform: ReleasePlatform, lastSeen: string | null): boolean;
export class ReleaseNotesSeenState { shouldAutoPresent(): boolean; markCurrentRendered(): void; }
export function releaseVersionLabel(release: ReleaseNote, platform: ReleasePlatform): string;
```

Import `../../../release-notes/releases.json` directly so the web bundle has no copied catalog. Log a malformed bundled catalog once, then return `null`.

**Step 3: Verify**

Run: `node --test web/tests/unit/release-notes.test.mjs`

Expected: PASS.

---

### Task 3: Svelte latest-first UI and entry points

**Files:**

- Create: `web/src/lib/components/shell/ReleaseNotesOverlay.svelte`
- Modify: `web/src/lib/stores/fullscreen-overlay.svelte.ts`
- Modify: `web/src/lib/components/shell/FullscreenOverlay.svelte`
- Modify: `web/src/lib/graphite/shell/GraphiteShell.svelte`
- Modify: `web/src/routes/settings/general/+page.svelte`
- Modify: `web/src/lib/commands/index.ts`
- Modify: `web/src/lib/command-manifest.json` (generated)
- Create: `web/tests/unit/release-notes-ui-contract.test.mjs`
- Create: `web/tests/e2e/release-notes.spec.ts`

**Step 1: Write failing UI contract tests**

Assert the real sources/registry expose:

- overlay kind `release-notes` and `openReleaseNotesOverlay()`;
- current detail plus New/Fixed/Important headings, Done, and conditional older count;
- history row/detail/back controls and unavailable state;
- Settings About “What’s New” button;
- shared command id/verb `whats-new` running the same overlay action;
- shell auto-open initialization calling the pure seen-state helper.

Run: `node --test web/tests/unit/release-notes-ui-contract.test.mjs`

Expected: FAIL before the UI exists.

**Step 2: Extend the overlay shell**

Add `release-notes` to `OverlayKind`, export `openReleaseNotesOverlay()`, and render `ReleaseNotesOverlay` inside the existing fullscreen shell. Preserve the shell’s capture-phase Esc close behavior and overlay z-index.

**Step 3: Build the latest-first component**

`ReleaseNotesOverlay.svelte` owns only presentation state:

- `current` starts visible;
- “View older releases (N)” opens a newest-first history excluding current;
- a row opens the reusable detail surface; Back returns to history;
- empty sections are omitted; Important uses warning styling;
- unavailable catalog shows quiet fallback + Done;
- focus moves to the heading on mount and restores on destroy;
- `markCurrentRendered()` runs only after a valid current detail mounts.

Use semantic `h1`/`h2`/`ul` markup and native buttons/links.

**Step 4: Wire automatic and manual entry points**

- In `GraphiteShell.svelte`, after mount/hydration, instantiate `ReleaseNotesSeenState` with `localStorage`; open once when its decision is true.
- In Settings General, add an About section with dynamic platform label and a “What’s New” button.
- Register `whats-new` in the existing command registry next to other navigation surfaces.
- Regenerate the manifest: `pnpm --dir web generate:commands`.

**Step 5: Add browser behavior coverage**

`web/tests/e2e/release-notes.spec.ts` must cover manual open, older list/detail/back, Done/Esc, and localStorage suppression. Seed/clear `tesela:releaseNotes:lastSeen:web` with Playwright before navigation so tests are deterministic.

**Step 6: Verify serially**

Run: `node --test web/tests/unit/release-notes-ui-contract.test.mjs`

Expected: PASS.

Run: `pnpm --dir web check`

Expected: zero Svelte/type errors.

Run: `pnpm --dir web test:unit`

Expected: PASS.

Run: `pnpm --dir web test:e2e -- release-notes.spec.ts`

Expected: PASS.

---

### Task 4: Tauri platform identity and desktop release path

**Files:**

- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `scripts/desktop-release.sh`
- Modify: `web/tests/unit/desktop-build-contract.test.mjs`

**Step 1: Write failing desktop contracts**

- Rust test: the initialization script sets `window.__TESELA_PLATFORM__ = 'desktop'` and retains the empty same-origin API base.
- Node contract: `desktop-release.sh` validates catalog version `0.1.2`, renders curated notes, emits updater JSON through `JSON.stringify`/the tested helper rather than string interpolation, and prints `gh release create ... --notes-file` rather than `--generate-notes`.

Run: `cargo test -p tesela-desktop`

Expected: new assertion fails before the platform bridge exists.

Run: `node --test web/tests/unit/desktop-build-contract.test.mjs`

Expected: new release-note assertions fail.

**Step 2: Implement the bridge and release bump**

Extract `desktop_initialization_script() -> &'static str`, use it in `build_main_window`, and cover it without launching a webview. Bump `src-tauri/tauri.conf.json` from `0.1.1` to `0.1.2` so the current catalog pointer and release artifact agree.

**Step 3: Make desktop release notes deterministic**

Before any build:

```bash
node scripts/changelog.mjs validate --platform desktop --version "$version"
node scripts/changelog.mjs render --release "$release_id" --format markdown
node scripts/changelog.mjs render --release "$release_id" --format plain
```

Write Markdown to `dist/desktop/release-notes.md`, pass plain notes into the updater manifest via safe JSON serialization, and print a GitHub command using `--notes-file`. Remove the freeform `DESKTOP_RELEASE_NOTES` runtime source.

**Step 4: Verify**

Run: `cargo test -p tesela-desktop`

Expected: PASS.

Run: `node --test web/tests/unit/desktop-build-contract.test.mjs`

Expected: PASS, including quote/Unicode/newline serialization.

Run: `scripts/desktop-release.sh --skip-notarize`

Expected: validates the catalog/version before safely exiting or packaging an existing artifact; no signing key is printed.

---

### Task 5: Shared Swift catalog, presenter, and UI

**Files:**

- Create: `app/Tesela-iOS/Sources/Data/ReleaseNotes.swift`
- Create: `app/Tesela-iOS/Sources/Data/ReleaseNotes.json`
- Create: `app/Tesela-iOS/Sources/Views/ReleaseNotesView.swift`
- Create: `app/Tesela-iOS/Tests/ReleaseNotesTests.swift`
- Create: `scripts/check-release-notes-drift.sh`
- Modify: `app/Tesela-iOS/Sources/Graphite/Shell/GrAppShell.swift`
- Modify: `app/Tesela-iOS/Sources/Graphite/Views/GrSettingsView.swift`
- Modify: `app/Tesela-iOS/Sources/Views/AppShell.swift`
- Modify: `app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift`

**Step 1: Write failing native domain tests**

Cover:

- schema-1 decode, unknown schema rejection, and fail-soft bundled load;
- platform filtering/current slicing;
- missing, unknown, older, current, and newer/downgrade last-seen branches;
- dynamic `CFBundleShortVersionString` + `CFBundleVersion` formatting;
- presenter auto-open once, manual open, and mark-seen-after-render semantics using a suite-scoped `UserDefaults`.

Run: `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17' -only-testing:TeselaTests/ReleaseNotesTests`

Expected: FAIL because the native release-note types do not exist.

**Step 2: Implement native schema and selection logic**

Define:

```swift
struct ReleaseNotesCatalog: Decodable, Equatable
struct ReleaseNote: Decodable, Equatable, Identifiable
enum ReleaseNotesSource { static func loadBundled() -> ReleaseNotesCatalog? }
enum ReleaseNotesSelection {
    static func history(in catalog: ReleaseNotesCatalog, platform: String) -> [ReleaseNote]?
    static func shouldPresent(in catalog: ReleaseNotesCatalog, platform: String, lastSeen: String?) -> Bool
}
struct AppVersionLabel { static func display(info: [String: Any]) -> String }
@MainActor final class ReleaseNotesPresenter: ObservableObject
```

`ReleaseNotesPresenter` owns the loaded catalog, `isPresented`, last-seen key `releaseNotes.lastSeen.ios`, a session-memory seen ID, `autoPresentIfNeeded()`, `presentManually()`, and `markCurrentRendered()`.

**Step 3: Bundle canonical bytes safely**

Add an exact copy at `Sources/Data/ReleaseNotes.json`. `scripts/check-release-notes-drift.sh` uses `cmp` and prints the one copy command on mismatch. Run it in verification and release automation.

**Step 4: Implement reusable SwiftUI presentation**

`ReleaseNotesView` owns a `NavigationStack`:

- current detail first;
- conditional history link/count;
- older rows navigate to the same detail renderer;
- native back, Done, Dynamic Type, semantic lists, warning treatment;
- current detail `onAppear` invokes `markCurrentRendered`;
- malformed/unavailable catalog is represented by the presenter not opening automatically; manual action can show the quiet unavailable screen.

Add an `openReleaseNotes` environment action and a `releaseNotesPresentation(_:)` view modifier. Both `GrAppShell` and legacy `AppShell` create one `@StateObject ReleaseNotesPresenter`, attach the modifier after onboarding, and therefore share identical auto/manual/seen behavior.

**Step 5: Wire Settings About**

- Replace Graphite’s hard-coded `v0.4.1` footer with a dynamic version/build card and “What’s New” row.
- Replace legacy Settings’ hard-coded footer with the same dynamic value and environment action.

**Step 6: Verify**

Run: `scripts/check-release-notes-drift.sh`

Expected: PASS.

Run: `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17' -only-testing:TeselaTests/ReleaseNotesTests`

Expected: PASS.

---

### Task 6: Shared command, iOS release gate, and CI

**Files:**

- Modify: `app/Tesela-iOS/Sources/Data/CommandManifest.json` (generated copy)
- Modify: `app/Tesela-iOS/Sources/Graphite/Views/GrCommand.swift`
- Modify: `app/Tesela-iOS/Sources/Graphite/Shell/GrAppShell.swift`
- Modify: `app/Tesela-iOS/Tests/GrCommandTests.swift`
- Modify: `scripts/ios-testflight.sh`
- Modify: `web/package.json`
- Modify: `.github/workflows/ci.yml`

**Step 1: Write failing iOS command tests**

Extend `GrCommandTests` so `whats-new` appears from the generated manifest only when `executableIds` contains it, and verify its label/keywords come from the shared manifest.

Run: `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17' -only-testing:TeselaTests/GrCommandTests`

Expected: FAIL before the executable map is updated.

**Step 2: Regenerate and dispatch the command**

Run `pnpm --dir web generate:commands`, copy `web/src/lib/command-manifest.json` byte-for-byte to the iOS resource, add `whats-new` to `GrCommand.executableIds`, and dispatch it in `GrAppShell.runCommand` after palette dismissal using the existing 350ms sheet-transition pattern.

**Step 3: Gate TestFlight against curated notes**

Immediately after `ios-testflight.sh` stamps `APP_VERSION`/`BUILDNO` and before archive:

```bash
scripts/check-release-notes-drift.sh
node scripts/changelog.mjs validate --platform ios --version "$APP_VERSION" --build "$BUILDNO"
node scripts/changelog.mjs render --release "$release_id" --format plain > "$OUT/release-notes.txt"
```

Print the artifact path for App Store Connect copy. Validation failure aborts before archive/upload.

**Step 4: Add local/CI gates**

- Add web script `check:changelog` running full + web-pointer validation and the iOS drift check.
- Run `pnpm --dir web check:changelog` in the existing web CI job before `svelte-check`.
- Do not add a second iOS CI workflow in this bead; `tesela-6hu` owns that broader scope.

**Step 5: Verify command and release integration**

Run: `pnpm --dir web check:manifest`

Expected: PASS.

Run: `scripts/check-command-manifest-drift.sh`

Expected: PASS.

Run: `pnpm --dir web check:changelog`

Expected: PASS.

Run: `node --test scripts/tests/changelog.test.mjs web/tests/unit/desktop-build-contract.test.mjs`

Expected: PASS.

---

### Task 7: Full gate, product QA, handoff, and scoped commit

**Files:**

- Modify: `.docs/ai/current-state.md`
- Create: `.docs/ai/phases/2026-07-15-changelog-screen-report.md`
- Update bead: `tesela-8h0`

**Step 1: Run automated gates serially**

```bash
node --test scripts/tests/changelog.test.mjs
pnpm --dir web check:changelog
pnpm --dir web check:manifest
pnpm --dir web check
pnpm --dir web test:unit
pnpm --dir web test:e2e -- release-notes.spec.ts
cargo test -p tesela-desktop
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'
git diff --check
```

Do not run web check/unit/E2E concurrently because they share `.svelte-kit` and test-result state.

**Step 2: Perform product-level QA**

- Hosted web: clear the web seen key, confirm one auto-open, Done/Esc, no reopen, Settings/command replay, history/detail/back.
- Desktop: build/run the actual Tauri shell or installed bundle, confirm desktop label/version and post-update identity; do not claim the installed-bundle gate if signing/build access blocks it.
- iOS Simulator: reset `releaseNotes.lastSeen.ios`, complete onboarding if needed, confirm one auto-open, swipe/Done, no reopen, both Settings routes, palette command, history navigation.
- Corrupt catalog behavior is covered by automated tests; do not ship a deliberately corrupt resource to a signed app.

**Step 3: Record evidence and close durable state**

Write the phase report with changed surfaces, exact test results, product QA evidence, and any explicit human-only residual gate. Clear `.docs/ai/current-state.md` Plan if complete. Close:

```bash
bd close tesela-8h0 --reason "Bundled cross-platform What’s New UI, release tooling, automated coverage, and product QA completed."
```

**Step 4: Commit only the bead’s paths**

Review `git status` and `git diff --stat`; exclude `.harness/` and every unrelated `crates/tesela-sync/src/engine/loro_engine/` edit. Create one commit:

```text
feat(changelog): add cross-platform What's New experience (tesela-8h0)
```

**Step 5: Deliver the required QA checklist**

The final response must include exact click/key paths, observable outcomes, Done/Esc/swipe/cancel paths, seen/downgrade edge cases, and adjacent Settings/command/updater regressions. Do not push.
