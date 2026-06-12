# Tesela Backlog Loop Prompt

You are running one headless `ralph` iteration in `/Users/tfinklea/git/tesela`.

Self-identify your tier before work. If you are Codex/GPT-5, treat yourself as Senior (T2). If your model is lower than an item's `tier_floor`, stop and report instead of starting it.

## Non-negotiable startup

1. Read `AGENTS.md`.
2. Read `.docs/ai/current-state.md`, `.docs/ai/roadmap.md`, and `.docs/ai/phases/2026-06-12-codex-pi-batch-report.md`.
3. Run `git log --oneline -5` and `git status --short --branch`.
4. If the working tree is dirty before you start, inspect the diff. If it is unrelated or unclear, stop and report.

## Item selection

Pick exactly ONE unchecked item from roadmap section `### Codex/pi mono coordinator batch (2026-06-12)`.

Allowed:
- `tier_floor` is `junior` or `senior`
- `complexity` is `S` or `M`
- the item is not marked `ESCALATE`
- the files are outside the off-limits list below

Forbidden:
- `tier_floor: lead`
- `complexity: XL`
- any `ESCALATE (Opus/Fable)` item
- sync hot path, RelayTicker behavior, pairing, FFI/UniFFI, generated bindings, signing, TestFlight, `project.yml`, real mosaic data, or large refactors

If no allowed unchecked items remain, mark the current-state loop complete and stop.

## Work rules

- Implement only the selected item.
- Read referenced files before editing.
- Touch only files listed by the selected item unless the item explicitly requires a new test file.
- Do not push.
- Do not run against the live mosaic. Use temp/sandbox data under `/tmp` if runtime verification needs a server.
- Keep the change small and reviewable for Opus/Fable inheritance.

## Verification

Run the selected item's `Verify` command(s) exactly. If a command is stale or impossible, stop and record the reason in the report; do not substitute broad verification without explaining it.

## Handoff updates

After implementation and verification:

1. Mark the selected roadmap checkbox `[x]` only if its Verify passed.
2. Append/update `.docs/ai/phases/2026-06-12-codex-pi-batch-report.md` with:
   - item name
   - what landed
   - commit sha placeholder before commit, then actual sha after commit if possible
   - exact Verify result
   - any shakiness, TODO, or follow-up Opus/Fable should check
3. Keep `.docs/ai/current-state.md` short. Preserve the single RALPH BACKLOG LOOP plan item unless the batch is complete.
4. Commit one atomic change covering implementation plus handoff docs. Commit message format:
   - `fix(web): ...`
   - `test(ios): ...`
   - `fix(tui): ...`
   - or equivalent scoped conventional message
5. Stop. The next `ralph` iteration will pick the next backlog item.

If the item fails:
- Leave its roadmap checkbox `[ ]`.
- Add `<!-- failed 2026-06-12: <brief reason> -->` directly under the item.
- Update the report honestly.
- Commit only if you changed docs to record the failure; otherwise stop without committing.
