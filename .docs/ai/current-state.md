# Current State

*Last updated: 2026-04-09*

## Active Branch

`main`

## Architecture at a Glance

- **Rust workspace** (`crates/`): `tesela-core`, `tesela-cli` (`tesela`), `tesela-tui`, `tesela-mcp`, `tesela-server`, `tesela-plugins`. Stable, well-tested.
- **Web client** (`web/`): Next.js 16 App Router + React 19 + CodeMirror 6 + shadcn/ui. Under active development.
- TypeScript types for the web client are generated from Rust via `ts-rs` — run `cargo test -p tesela-core --lib export_bindings`.

**Design quality bar:** Linear × Logseq × Zed — clean, intentional, keyboard-first, dark-mode-first, monochrome + single accent, type-led hierarchy.

**Plan file:** `/Users/tfinklea/.claude/plans/async-giggling-moth.md`

## Active Milestone

**M1 — Read-only outliner** (not started)

See `.docs/ai/roadmap.md` for the full M0–M8 list. M0 (scaffold & connect) is done.

M1 scope:
- Decide: port `BlockParser` to TS, or expose `tesela-core::block::parse_blocks` via a new `/notes/:id/blocks` endpoint
- `/p/[id]` route rendering a note's blocks in an indented outliner layout
- One CM6 instance per block (read-only), decorations for `[[wiki-links]]`, `#tags`, `key:: value` properties
- Arrow-key navigation between blocks

## Last Session Summary

**Date**: 2026-04-09

- **Completed M0 — Scaffold & Connect:**
  - Added `ts-rs` v12 (dev-dep) to `tesela-core`; annotated 11 types across 4 files with `#[cfg_attr(test, derive(TS))]`. Used `#[ts(type = "Record<string, unknown>")]` on `NoteMetadata.custom` to avoid the `serde_json::Value` cross-crate import.
  - Scaffolded `web/` with Next.js 16.2.3 App Router, TypeScript, Tailwind v4, shadcn/ui (base-nova preset on `@base-ui/react`, neutral base color). shadcn no longer uses Radix.
  - Installed CodeMirror 6 core + lang-markdown + search, `@replit/codemirror-vim`, TanStack Query v5, Zustand v5, cmdk, Lucide.
  - Built `web/src/lib/api-client.ts` (typed `ApiClient` class) and `web/src/lib/ws-client.ts` (exponential backoff reconnect, `intentionallyStopped` latch, connection-id guard).
  - Root layout forces `dark` class, wires `Providers` (TanStack Query + WS auto-connect). Boot screen at `/` renders header, status pill, notes list, and error/empty/loading states.
  - Verified via Chrome DevTools MCP: page loads at `http://localhost:3000`, title "Tesela", dark theme applies, api-client fires, WS reconnect backs off correctly, expected error state renders with the `cargo run -p tesela-server` hint. `pnpm tsc --noEmit` and `pnpm lint` clean.
- **Dropped legacy SwiftUI macOS app.** The entire `app/` directory was deleted. SwiftUI references scrubbed from `README.md`, `CLAUDE.md`, `scripts/release.sh`, `.docs/ai/roadmap.md`, and `.claude/agents/test-writer.md`. The CI + release workflows were already Rust-only, so they needed no changes.

## Build Status

- Web: `pnpm --dir web tsc --noEmit` clean, `pnpm --dir web lint` clean, `next dev` boots in ~290ms
- Rust: `cargo test -p tesela-core --lib export_bindings` green (11 tests)
- Full `cargo test --workspace` verification pending after the SwiftUI cleanup sweep

## Blockers

None.

## Next Phases

- **M1–M8**: Web client build-out (see `.docs/ai/roadmap.md`)
- **Rust-side backlog**: Still active — see the Backlog section in `roadmap.md`
