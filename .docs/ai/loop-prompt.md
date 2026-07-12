# Tesela phase-loop prompt

You are one fresh-context iteration of the plan in
`.docs/ai/current-state.md`. Execute exactly one phase; never scan the old
roadmap backlog for a different item.

## Startup

1. Run `bd prime`.
2. Read `AGENTS.md`, `.docs/ai/current-state.md`, and only the roadmap `Now`
   section needed to understand the active product arc.
3. Read the phase spec referenced by the first unchecked `## Plan` item.
4. Run `git log --oneline -5` and `git status --short --branch`. Stop if the
   tree contains an unrelated or unclear change.
5. Self-check the item's `tier_floor`. Current MiniMax M3 and GLM-5.2 roster
   lanes are Senior; a model below the floor stops without editing.

## Selection and work

- Pick exactly the first unchecked item in `.docs/ai/current-state.md`'s
  `## Plan`. Do not use `bd ready`, roadmap Backlog, or a historical batch to
  substitute another task.
- Read every referenced file before editing. Follow the phase spec and existing
  code patterns; do not invent adjacent scope.
- Use TDD: add the focused failing test, confirm the expected failure,
  implement the smallest production change, then run the item's exact
  `Verify:` command.
- Never read or write the live mosaic. Runtime fixtures use temp/sandbox data;
  `~/logseq` is read-only and may only be copied when the phase explicitly says
  so.
- You are in the shared tree. Do not run `git stash`, `git checkout`,
  `git reset`, or mutate git state for diagnostic comparisons. Do not push.

## Close the iteration

1. If Verify passes, mark only that Plan checkbox `[x]`. If a named human check
   remains, mark it `[?] awaiting human verify`. On failure, leave `[ ]` and
   record the concise blocker without guessing.
2. Keep `.docs/ai/current-state.md` at 20 lines or fewer. Record durable design
   rationale only in `.docs/ai/decisions.md`. File newly discovered actionable
   work in beads with metadata and a `discovered-from` dependency; do not add a
   prose backlog duplicate.
3. Make one atomic commit covering implementation, tests, and handoff-state
   updates. Do not push.
4. Stop. Do not start the next phase and do not run `chezmoi apply`.
