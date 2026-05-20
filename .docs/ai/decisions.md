# Architecture Decision Records

Concise log of non-obvious decisions. Newest first.

---

### 2026-03-30 — Apple-first, web later (platform strategy)

**Decision:** SwiftUI/AppKit is the primary GUI. Use SF Symbols for icons. A Tauri/web app can be built later sharing the Rust backend API, with its own icon set (Tabler/Lucide) mapped from the same frontmatter `icon` field.

**Why:** Taylor is the sole user on macOS. Native AppKit gives the best keyboard-first editing experience. The Rust backend already runs cross-platform. Rewriting in a shared UI framework now would kill momentum for hypothetical users.

**Trade-off:** Two GUI codebases eventually. But the server API is the shared contract, and different icon libraries per platform is normal (like VS Code vs native IDEs).

---

### 2026-03-27 — Keyboard-navigable select popover (SelectListView)

**Decision:** Replace NSButton-based select popovers with a custom NSView subclass that handles keyDown (arrow/j/k, Enter, Escape) and mouse clicks.

**Why:** Mouse-only popovers broke the keyboard-first UX promise. NSMenu alternatives had target deallocation issues.

**Trade-off:** More custom AppKit code to maintain, but consistent with the Vim-everywhere philosophy.

---

### 2026-03-27 — Preserve caller frontmatter in store.create()

**Decision:** If content passed to `FsNoteStore::create()` already starts with `---`, write it as-is instead of prepending auto-generated frontmatter.

**Why:** Property and Tag pages created from the SwiftUI app include custom frontmatter (type, value_type, choices). The old behavior doubled the frontmatter block and lost those fields.

**Trade-off:** Callers that pass frontmatter are responsible for including `created` timestamps themselves.

---

### 2026-03-25 — Properties and Tags as pages, not config files

**Decision:** Adopt Logseq DB model — Tags, Properties, and Values are all markdown pages with YAML frontmatter. No more `types.toml`.

**Why:** "Everything is a page" aligns with Tesela's file-based philosophy. Users can browse, edit, and link to type definitions. Enables property inheritance through `extends` chains.

**Trade-off:** Server must understand Property/Tag page semantics. More complex indexing. But files remain the single source of truth.

---

### 2026-03-20 — Database-first architecture shift

**Decision:** TUI becomes an API client. Local tesela-server runs always. Central server planned for sync.

**Why:** SwiftUI app already uses REST API. Having TUI also use the API means one code path for all clients. Enables future multi-device sync.

**Trade-off:** TUI loses offline-only simplicity. But the server is local, so it's effectively the same.

---

### 2026-03-15 — Custom NSTextView outliner, not embedded Neovim

**Decision:** Build the block editor from scratch using one NSTextView per block inside an OutlinerView (NSView), wrapped in a single NSViewRepresentable.

**Why:** Embedded Neovim can't do block-aware motions (j/k between blocks, dd deletes block, >> indents hierarchy). WKWebView+TipTap adds web complexity. Native AppKit gives full control over Vim integration.

**Trade-off:** Significant upfront work for the editor. But it's the right long-term investment for keyboard-first UX.

---

### 2026-05-19 — iOS bottom chrome: native TabView with `Tab(role: .search)`, not a custom HStack

**Decision:** Use SwiftUI's `TabView` with `Tab(_:systemImage:value:)` for place-tabs (Daily/Inbox/Library) and `Tab(value:role:.search)` for the search slot. The system manages all visual chrome — pill geometry, Liquid Glass material, safe-area positioning, scroll-edge effects, the trailing-pinned search circle, accessibility. Capture stays a sheet trigger from the TopBar (no bottom-chrome slot).

**Why:** A hand-rolled `BottomChrome` HStack of `glassEffect` shapes was nominally correct but visually wrong — wrong height, wrong baseline above the home indicator, wrong selection treatment, and the three glass shapes refracted inconsistently because each had its own sampling region. Reference apps (Simmersmith, Seedkeep, Joji) all use plain native `TabView` and look correct effortlessly. Phone/Mail iOS 26's trailing search-circle look is `Tab(role: .search)`, which the system pins as a standalone Liquid Glass circle separate from the labeled pill.

**Trade-off:** No way to put a second standalone glass shape (e.g., a capture FAB) next to the search circle without abandoning the native chrome entirely. iOS 26 has only `.search` as a `TabRole`; `tabViewBottomAccessory` always renders as a row *above* the tab bar at rest on iPhone (docs: "the accessory appears above [the tab bar at normal size]; inline only when the tab bar is collapsed"). A custom three-shape `BottomChrome` was prototyped — it works but doesn't visually match Apple's chrome, so it was reverted. Capture lives in `DailyTopBar`'s icon row via the new `\.openCapture` environment value.

---

### 2026-05-20 — One process-wide `EKEventStore`, not one per operation

**Decision:** All EventKit access in `reminders/darwin.rs` goes through a single lazily-created `EKEventStore` held in a module `OnceLock` (`shared_event_store()`). Push, pull, and the access request previously each constructed their own.

**Why:** EventKit caps how many `EKEventStore` instances a process may hold. Each `sync_all` built four (the access request runs inside both `pull_all` and `push_all`), so auto-sync every 5 minutes exhausted the cap within ~an hour — EventKit then rejected every call with "too many EKEventStore instances. Use fewer event stores". A shared store keeps the live-instance count at exactly one.

**Trade-off:** The `Retained<EKEventStore>` is parked in a `static` behind an `unsafe impl Send + Sync` wrapper. That is sound only because every EventKit call is already serialized by `AutoSync`'s in-flight mutex — the store is never touched from two threads at once. A future caller that hits `push_all`/`pull_all` outside that mutex would break the wrapper's safety justification.

---

### 2026-05-20 — iOS on-device Parakeet ASR via the FluidAudio package

**Decision:** On-device Parakeet transcription is provided by the FluidAudio Swift package (`FluidInference/FluidAudio`). `LocalTranscriptionEngine` dispatches by model family — Whisper stays on SwiftWhisper, Parakeet routes to a FluidAudio `AsrManager`. FluidAudio owns Parakeet model download + caching (`AsrModels.downloadAndLoad`), so `TranscriptionCatalog`'s Parakeet entries carry no `downloadURL`; a `parakeetVersion` token (`v2` / `v3` / `tdtCtc110m`) maps to `AsrModels.Version`. Tesela passes a per-version cache directory under Application Support so `deleteModel` can remove the files.

**Why:** The catalog's old Parakeet `.zip` URLs 404'd and nothing ran inference. FluidAudio ships the same `parakeet-tdt-0.6b` CoreML build VoiceInk and Handy use and manages its own model download — far cheaper than hand-rolling a NeMo runtime.

**Trade-off:** FluidAudio's `downloadAndLoad` exposes no progress, so a Parakeet download shows an indeterminate spinner, not a percentage. The package is pinned to `branch = main` (no release tag). Whisper is completely untouched — it remains the URLSession-`.bin` path.
