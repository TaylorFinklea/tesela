# Tesela Senior/Junior Backlog Loop Prompt

You are running one headless `ralph` iteration in `/Users/tfinklea/git/tesela`.

Self-identify your tier before work.
- If you are `kimi-k2.7-code`, `gpt-5.5`, `gpt-5`, Codex, or another equivalent frontier coding model, treat yourself as **Senior (T2)**.
- If you are `minimax-m3`, `minimax`, or a similar fast mechanical model, treat yourself as **Junior (T3)**.

If you are below an item's `tier_floor`, stop and report instead of starting it.

## Non-negotiable startup

1. Read `AGENTS.md`.
2. Read `.docs/ai/current-state.md`, `.docs/ai/roadmap.md`, and `.docs/ai/phases/command-registry-spec.md`.
3. Run `git log --oneline -5` and `git status --short --branch`.
4. If the working tree is dirty before you start, inspect the diff. If it is unrelated or unclear, stop and report.

## Item selection

Pick exactly ONE unchecked backlog item from `.docs/ai/roadmap.md`.

Priority order:
1. Items in `### 2026-06-13 Ralph batch — command registry foundation (keyboard-first spine)`.
2. Senior-safe unchecked items in `### Opencode-ready reliability polish (2026-06-12)` (if any remain).
3. Senior-safe unchecked items in `### Codex/pi mono coordinator batch (2026-06-12)` (if any remain).
4. Any later `## Backlog` item only if it has the full fields below and is clearly inside the safe zones.

Tier routing:
- **Senior models** may pick items with `tier_floor: senior` or `tier_floor: junior`.
- **Junior models** may pick items with `tier_floor: junior` only. Do NOT wait for senior items to complete; junior and senior items may run in parallel.
- If no item at your tier remains, stop and report "batch complete at tier X".

Allowed:
- `complexity` is `S`, `M`, or `L` (senior models only for `L`)
- the item has explicit `Scope`, `Files`, `Acceptance`, and `Verify` fields
- the item is not marked `ESCALATE`
- the files are outside the off-limits list below

Forbidden:
- `tier_floor: lead`
- `complexity: XL`
- any `ESCALATE (Opus/Fable)` item
- sync hot path, RelayTicker behavior, pairing, FFI/UniFFI, generated bindings, signing, TestFlight, `project.yml`, real mosaic data, or large refactors
- old mechanical backlog bullets that do not carry `Scope` / `Files` / `Acceptance` / `Verify` / `tier_floor` / `complexity`

For the 2026-06-13 batch, prefer the item whose `ralph_model` matches your model when multiple items are available at your tier. If none match, take the next available item at your tier.

## Work rules

- Implement only the selected item.
- Read referenced files before editing.
- Touch only files listed by the selected item unless the item explicitly requires a new test file.
- Do not push.
- Do not run against the live mosaic. Use temp/sandbox data under `/tmp` if runtime verification needs a server.
- Keep the change small and reviewable for Opus/Fable inheritance.
- If an item is marked `manual` in its Verify, run the automated parts and report the manual checklist as "awaiting human verify".

## Verification

Run the selected item's `Verify` command(s) exactly. If a command is stale or impossible, stop and record the reason in the report; do not substitute broad verification without explaining it.

## Handoff updates

After implementation and verification:

1. Mark the selected roadmap checkbox `[x]` only if its Verify passed.
2. Append/update `.docs/ai/phases/2026-06-13-ralph-batch-report.md` with:
   - item name
   - what landed
   - commit sha placeholder before commit, then actual sha after commit if possible
   - exact Verify result
   - any shakiness, TODO, or follow-up Opus/Fable should check
3. Keep `.docs/ai/current-state.md` short. Update the single RALPH BACKLOG LOOP plan item checkbox.
4. Commit one atomic change covering implementation plus handoff docs. Commit message format:
   - `fix(clippy): ...`
   - `feat(web): ...`
   - `refactor(web): ...`
   - or equivalent scoped conventional message
5. Stop. The next `ralph` iteration will pick the next backlog item.

If the item fails:
- Leave its roadmap checkbox `[ ]`.
- Add `<!-- failed 2026-06-13: <brief reason> -->` directly under the item.
- Update the report honestly.
- Commit only if you changed docs to record the failure; otherwise stop without committing.