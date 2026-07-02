# Tesela Architecture

> ⚠️ **HISTORICAL (Phase 8, ~2026-03).** This document predates the Loro
> cutover and is kept for context only. It still says "files are truth,
> database is cache" and describes a SqliteEngine / dual-write model + a
> planned Slint GUI — all superseded. The current model: **Loro CRDT is the
> sync source of truth**, Markdown files are a materialized export, and SQLite
> is a rebuildable cache; clients are SvelteKit web + SwiftUI iOS + a Tauri
> desktop shell. For the live picture use `AGENTS.md`, `.docs/ai/roadmap.md`,
> `.docs/ai/current-state.md`, and `.docs/ai/decisions.md` (2026-05-29 /
> 2026-06-10), not this file.
