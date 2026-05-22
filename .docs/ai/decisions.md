# Architecture Decision Records

Concise log of non-obvious decisions. Newest first.

---

### 2026-05-21 — Workhorse/spark accent split; the spark is a theme, not a rule

**Decision:** `--accent-primary` is an earthy terracotta (`#E07A5F`) — the everyday accent for links, bullets, selection. The neon coral (`#FB5950`) is a separate `--accent-spark` token. `--accent-spark` defaults to `var(--accent-primary)`, so standard themes show no neon; only the opt-in **Prism Spark** theme overrides it (`[data-theme="prism-spark"] { --accent-spark: #fb5950 }`). iOS mirrors this with a `Theme.accentSpark` computed property keyed on `id == .prismSpark`.

**Why:** The logo coral at full saturation, used as the app-wide accent, read as harsh — a hot hue hit hundreds of times per screen is noise, not accent. Splitting a calm workhorse from a rare neon spark is the standard hero-vs-workhorse colour split. Making the spark a *theme toggle* rather than a hardcoded set of spots means the two variants can't drift apart — Prism Spark is definitionally "Prism + one token".

**Trade-off:** Three Prism themes now (Prism, Prism Spark, Prism Light) where one might do. But the spark is a genuine taste call, and a one-token override theme is nearly free to maintain. The logo keeps the neon `#FB5950` — a logo is a stamp seen briefly, the app a surface stared at, so they legitimately use the same hue at different saturations.

---

### 2026-05-21 — The v4/v5 chrome token layer aliases the role tokens

**Decision:** `web/src/lib/v4/tokens.css` `--v4-*` tokens (`--v4-bg`, `--v4-ink*`, `--v4-surface*`, `--v4-hair*`, `--v4-accent*`) now alias the role tokens (`var(--bg)`, `var(--fg-*)`, …) instead of being hardcoded literals. The `.v4-root`'s old `--accent-primary: var(--v4-accent)` override is removed.

**Why:** The live app's chrome (the `/v4` route — `BufferShell`, `Sidebar`, `StatusLine`, the `.v4-root` shell) reads `--v4-*`, which were hardcoded dark literals (`--v4-bg: #131521`, near-white ink) decoupled from the 30-theme role-token system. So *no theme changed the chrome*, and light themes appeared completely broken (background frozen dark). The `--v9-*` layer already aliased the role tokens; `--v4-*` should have too.

**Trade-off:** The v4 ink ramp had six levels; the role contract has four, so the two finest gradations double up (minor loss of text-hierarchy nuance). The dark Prism chrome's surface shifts from the old `#131521` to the real Prism `#23252F` — a visible change, but a correct one: the chrome had never actually used the designed Prism palette.

---

### 2026-05-21 — Default theme rebranded to warm-dark "Prism"; light variant ships

**Decision:** The app's default theme is a new warm-dark **Prism** palette derived from the locked logo — slate surfaces (`#23252F`→`#3D405B`), cream text (`#F4F1DE`), coral brand accent (`#FB5950`), sage secondary (`#81B29A`) — replacing the placeholder indigo `#7b8cff`. A **Prism Light** theme (cream / slate / deepened-coral `#DD4A3D`) ships alongside it. The cross-platform theme id `prism-indigo` becomes `prism`; the web FOUC default and the iOS `@AppStorage` default both move to `prism`.

**Why:** The logo was finalized as a two-tone mosaic mark; the app's chrome should agree with its own logo. The indigo `#7b8cff` was a leftover from the v4 proto mockup with no logo behind it. Keeping Prism *warm-dark* (rather than a straight light translation of the cream logo) preserves Tesela's dark-mode-first stance while every surface and the accent now harmonize with the mark.

**Trade-off:** (1) `accent-secondary` (sage `#81B29A`) is an extrapolation — a two-tone logo supplies only one accent, but the role-token contract needs a navigational secondary; sage is the natural completion of the logo's source terracotta palette and is chosen to recede rather than compete with coral. (2) Shipping Prism Light **supersedes** the earlier "always dark on first ship, light themes land later" decision (iOS design-followup #10); `preferredColorScheme` now tracks `Theme.isLight`. (3) The Swift enum case was renamed `prismIndigo`→`prism`; any persisted `"prism-indigo"` preference falls through to the new `prism` default — acceptable, since those users were on the default anyway. (4) The legacy `.v4-root` route keeps its own `#131521` surface and ink ramp; only its accent tokens were repointed to coral/sage.

---

### 2026-05-20 — `tesela-server` bind is config-driven; default stays loopback

**Decision:** Add a `[server] bind` key (`ServerConfig` in `tesela-core`). `tesela-server` resolves its bind address as `TESELA_SERVER_BIND` env → `[server].bind` in the global config → `127.0.0.1:7474`. The compiled default stays loopback; LAN exposure is opt-in per machine via config. Taylor's `~/.config/tesela/config.toml` sets `0.0.0.0:7474`.

**Why:** iOS↔desktop sync was impossible from a physical device because the server only ever bound loopback — reachable from the iOS simulator (shared host network) but not a real iPhone. The bind *must* live in config, not just the env var: `/server/restart` (used by iOS mosaic-switching) re-execs the binary without inheriting the environment, so an env-only bind would silently revert to loopback after every switch.

**Trade-off:** `0.0.0.0` exposes the server — which has no auth by default — to every device on the LAN. Acceptable for a single-user daily driver on a trusted network (the same posture as Syncthing/Logseq sync), but a coffee-shop Wi-Fi is genuinely unsafe until an auth token is enforced. Keeping the *compiled* default at loopback means only machines that explicitly opt in are exposed. `MosaicProfile.authToken` exists but is not yet checked server-side.

**Follow-up (same day):** even with `0.0.0.0`, the iPhone still couldn't reach the Mac's plain LAN IP — both devices are on the user's Tailscale tailnet, which advertises `10.x` subnet routes, so the phone routed the LAN subnet into the Tailscale tunnel. The reliable address is the Tailscale IP (`100.64.0.0/10`). `first_lan_ipv4()` now prefers a Tailscale CGNAT address when one exists, so pairing codes advertise the Tailscale IP automatically; it falls back to a plain LAN IP otherwise. Lesson: for a multi-device personal setup the overlay-network address is more reliable than the physical LAN IP.

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
