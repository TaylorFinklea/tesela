# Current State

*Last updated: 2026-04-09*

## Active Branch

`main`

## Major Decision: Web Frontend Pivot (2026-04-09)

The primary desktop client is pivoting from SwiftUI to a Next.js + React + CodeMirror 6 web app. **SwiftUI is now frozen** — it stays in the repo but receives no new feature work. A Rust web client (Leptos/Dioxus) was evaluated and rejected: no mature Vim-capable rich-text editor exists in the Rust/WASM ecosystem, and DOM performance is the real bottleneck (not compute), so Rust's speed advantage doesn't translate to the browser.

**Why:** UI iteration was gated by manual QA — Claude cannot drive SwiftUI. A web client can be driven end-to-end by Chrome DevTools MCP, removing the bottleneck entirely. The server-thin-client architecture already in place means the pivot cost is only the UI layer.

**Plan file:** `/Users/tfinklea/.claude/plans/async-giggling-moth.md`

**Design quality bar:** Linear × Logseq × Zed — clean, intentional, keyboard-first, dark-mode-first, monochrome + single accent, type-led hierarchy.

## Active Milestone

**M0 — Scaffold & Connect** (complete, pending commit)

- [x] Update handoff docs
- [x] Add `ts-rs` to `tesela-core` for TS type export (committed in `465c6a8`)
- [x] Scaffold Next.js app in `web/` (Next.js 16.2.3, React 19.2.4, Tailwind v4)
- [x] Install M0 dependencies (CM6, @replit/codemirror-vim, TanStack Query, Zustand, cmdk, shadcn/ui on @base-ui/react, Lucide)
- [x] Build `web/src/lib/api-client.ts` and `web/src/lib/ws-client.ts`
- [x] Minimal boot screen: header, status pill (live/loading/offline), notes list, error/empty/loading states
- [x] `web/.gitignore` exists from create-next-app; parent repo tracks `web/` after removing nested `.git`
- [x] Verify via Chrome DevTools MCP (page loads, dark theme applies, api-client fires, error state renders, WS reconnect loop backs off)
- [ ] Commit M0 (in progress)

## Last Session Summary

**Date**: 2026-04-09

- Evaluated and executed pivot from SwiftUI to web frontend
- Explored tesela-server API surface: ~95% coverage, CORS permissive, no auth — web client is feasible today
- Inventoried SwiftUI app: 8,776 lines; 3K lines of NSTextView/AppKit in the outliner core is the porting bottleneck
- Researched Rust web UI options (Leptos, Dioxus, Yew), rejected due to Vim-editor ecosystem gap
- Selected stack: Next.js + React + CodeMirror 6 + shadcn/ui + Tailwind, with ported Swift VimEngine on top
- **Completed M0 — Scaffold & Connect:**
  - Added `ts-rs` v12 (dev-dep) to `tesela-core`; annotated 10 types across 4 files with `#[cfg_attr(test, derive(TS))]`. `cargo test -p tesela-core --lib export_bindings` writes TS definitions to `web/src/lib/types/`. Used `#[ts(type = "Record<string, unknown>")]` on `NoteMetadata.custom` to avoid the `serde_json::Value` cross-crate import.
  - Scaffolded `web/` with Next.js 16.2.3 App Router, TypeScript, Tailwind v4, shadcn/ui (base-nova preset, neutral base color). shadcn uses `@base-ui/react` now, not Radix.
  - Installed CodeMirror 6 core + lang-markdown + search, `@replit/codemirror-vim`, TanStack Query v5, Zustand v5, cmdk, Lucide.
  - Ported `WebSocketClient.swift` → `web/src/lib/ws-client.ts` preserving exponential backoff (1s→30s), `intentionallyStopped` latch, and a connection-id guard that prevents stale receive loops from stomping on fresh connections.
  - Built `api-client.ts` with typed `ApiClient` class (`health()`, `listNotes()`) and an `ApiError` class.
  - Root layout forces `dark` class, wires `Providers` (TanStack Query + WS auto-connect). Page.tsx boot screen renders header, status pill, notes list or loading/error/empty states.
  - Verified via Chrome DevTools MCP: page loads at `http://localhost:3000`, title is "Tesela", dark theme applies, api-client fires, WS reconnect loop hits tesela-server, expected error state renders with "Start it with `cargo run -p tesela-server`" hint. `pnpm tsc --noEmit` and `pnpm lint` both clean.
  - Removed two accidental cruft directories: the nested `web/.git` auto-created by `create-next-app`, and the `crates/web/` orphan from a wrong early `export_to` path.

## Pre-existing Uncommitted Work (leave alone)

- `app/Tesela/Tesela/Editor/OutlinerView.swift` — modified (-1400 lines)
- `app/Tesela/Tesela/Editor/OutlinerLayout.swift` — new (+647 lines)
- This is the failed OutlinerView split attempt from an external agent (memory noted it failed/reverted)
- Net ~750 lines unaccounted for — SwiftUI almost certainly doesn't build in this state
- **Decision**: leave it alone per Taylor's instruction. Do NOT stage these files in M0+ commits.

## Build Status

- Web: `pnpm tsc --noEmit` clean, `pnpm lint` clean, `next dev` boots in ~290ms
- Rust: `cargo test -p tesela-core --lib export_bindings` green (11 tests), rest of workspace needs verification
- SwiftUI: **broken** (orphan OutlinerView split) — frozen anyway, not blocking

## Blockers

None.

## Next Phases

- **M0–M8**: Web client build-out (see `.docs/ai/roadmap.md` Phase 1b)
- **SwiftUI phases**: Frozen
- **Rust-side backlog**: Still active, benefits both clients
