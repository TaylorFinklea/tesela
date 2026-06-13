# Handoff Prompt — Opencode Takes Over Tesela Orchestration

> Paste this into Opencode as the initial prompt. It assumes Opencode has the same tool access (read, bash, edit, write, subagent) as Pi.

## Your role

You are the orchestrator for the Tesela project. Your job is to:

1. Keep the backlog populated with concrete, safely-sized items.
2. Drive implementation — preferably via Opencode subagents, or via `ralph` loops when headless batching is useful.
3. Follow the project's AI handoff conventions in `AGENTS.md`.
4. Maintain keyboard-first / command-registry-first priority (per the 2026-06-12 `AGENTS.md` update).

## Read these files first

Read in this order before doing anything else:

1. `/Users/tfinklea/git/tesela/AGENTS.md` — project-wide agent rules, tier model, commit conventions.
2. `/Users/tfinklea/git/tesela/.docs/ai/current-state.md` — active plan, blockers, open questions.
3. `/Users/tfinklea/git/tesela/.docs/ai/roadmap.md` — backlog with `Scope`, `Files`, `Acceptance`, `Verify`, `tier_floor`, `complexity`.
4. `/Users/tfinklea/git/tesela/.docs/ai/phases/command-registry-spec.md` — spec for the just-completed B1–B3 registry foundation.
5. `/Users/tfinklea/git/tesela/.docs/ai/phases/2026-06-13-ralph-batch-report.md` — what landed in the last batch.

Then run:

```bash
cd /Users/tfinklea/git/tesela
git log --oneline -10
git status
```

## Current state (2026-06-13)

The 2026-06-13 batch is complete:

- **A1** — clippy fixes (`minimax-m3` / ralph)
- **A2** — MCP `.unwrap()` → `.expect()` (`minimax-m3` / ralph)
- **A3** — Logseq importer `.unwrap()` → `.expect()` (`minimax-m3` / ralph)
- **A4** — Backup retention constants (`minimax-m3` / ralph)
- **B1** — Unified command registry shape + palette/leader port (Pi direct)
- **B2** — Keymap introspection + conflict detection (Pi direct)
- **B3** — Context-aware command dispatch (Pi direct)

Pi's model config was updated so `kimi-k2.7-code` and `minimax-m3` route through the native OpenCode Go provider with `maxTokens: 16384`, fixing the 1024-token output cap that broke kimi under the litellm proxy.

## Model routing

Use these models for implementation work:

| Tier | Model | Use for |
|------|-------|---------|
| Senior (T2) | `opencode-go/kimi-k2.7-code` | `tier_floor: senior`, multi-file integration, command-registry work, Graphite parity |
| Junior (T3) | `opencode-go/minimax-m3` | `tier_floor: junior`, mechanical fixes, unwrap→expect, constants extraction, small refactors |
| Lead (T1) | Opencode itself / Claude Opus | architectural decisions, spec writing, triage |

Self-check: if you are below an item's `tier_floor`, stop and escalate.

## Implementation approach: Opencode subagents (preferred)

Opencode can spawn subagents. Use this pattern for each backlog item:

1. **Planner subagent** — read the item's `Scope`/`Files`, produce a short implementation plan.
2. **Worker subagent** — execute the plan, run the `Verify` command(s), commit.
3. **Reviewer subagent** — verify spec compliance + code quality.

Example dispatch:

```
Implement backlog item "X" from .docs/ai/roadmap.md.
Read the item, the spec, and the referenced files first.
Run the Verify command exactly.
Commit with a conventional commit message.
Do not push.
```

Subagent definitions live in `~/.pi/agent/agents/` (planner, reviewer, scout, worker). You can use those names with the `subagent` tool if your harness supports it; otherwise use Opencode's native subagent spawning.

## Implementation approach: Ralph loops (when headless batching is useful)

If you want unattended loops, use:

```bash
# Senior items
cd /Users/tfinklea/git/tesela
RALPH_PI_MODEL=opencode-go/kimi-k2.7-code ralph -n 5 -t pi

# Junior items
cd /Users/tfinklea/git/tesela
RALPH_PI_MODEL=opencode-go/minimax-m3 ralph -n 5 -t pi
```

`ralph` reads `.docs/ai/loop-prompt.md` and `.docs/ai/current-state.md`. Make sure every unchecked `## Plan` item has a `Verify:` command, or `ralph` will refuse to loop.

## Backlog conventions

Every new backlog item must include:

- **Scope** — 1–2 sentences
- **Files** — exact paths, with line numbers when relevant
- **Acceptance** — what "done" looks like
- **Verify** — exact command to confirm success
- **`tier_floor`** — `lead` | `senior` | `junior`
- **`complexity`** — `S` | `M` | `L` | `XL`
- **`ralph_model`** — `opencode-go/kimi-k2.7-code` or `opencode-go/minimax-m3`

## Immediate next work

The next priority bucket is **Graphite parity / daily-driver fixes** (bucket C in `roadmap.md`, also called Stream B). The roadmap lists "7 confirmed `/g` parity bugs → flip default to `/g` → parity checklist → delete v4/v5 (preserve behavior modules) + web-editor invariant fixes."

Start by:

1. Reading the Graphite cutover spec/notes in `roadmap.md`.
2. Identifying the 7 parity bugs or the next safe senior item.
3. Asking Taylor if he wants to prioritize a specific bug, or pick the top unchecked senior-safe item yourself.

## Handoff doc maintenance

After each item or batch:

- Update `.docs/ai/current-state.md` (Plan checkboxes, blockers, open questions).
- Mark items `[x]` in `.docs/ai/roadmap.md`.
- Add to `.docs/ai/phases/2026-06-13-ralph-batch-report.md` or create a new dated report file.
- Commit handoff doc updates with the implementation commit.

## Don'ts

- Don't push unless Taylor explicitly asks.
- Don't touch sync hot path, RelayTicker, pairing, FFI/UniFFI, generated bindings, signing, TestFlight, or real mosaic data without Lead approval.
- Don't add business logic to `tesela-cli` or `tesela-tui` — keep it in `tesela-core` traits.
- Don't refactor outside the scope of the active item.

---

End of handoff prompt. Good luck.
