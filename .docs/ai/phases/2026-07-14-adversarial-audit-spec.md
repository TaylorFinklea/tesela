# 2026-07-14 — Adversarial audit + 3-week direction (Fable orchestrating)

## Mission
Find architecture issues + bugs via multi-model adversarial review, populate beads, set the 3-week direction. Priority (Taylor): FINAL VERSION = perfected personal daily driver — feature depth + stability.

## Taylor's scoping answers (2026-07-14)
- V1 target: perfected daily driver (no new-audience gates yet)
- Hardening: desktop + iOS equally
- RTC epic tesela-680: FULL BUILD IN SCOPE — north-star arc continues in parallel with stability
- Pain (all of): sync trust · editor depth · iOS parity · tasks+agenda · widgets half-baked · properties below Logseq-DB/Anytype parity · bugs

## Method
1. Maps: 6 Sonnet Explore agents (core engine, sync/relay, web+desktop, iOS, quality/CI/hygiene, backlog themes) → each returns a reviewer dossier file list.
2. Dossiers: assembled into scratchpad files (inline source, file:line anchored) for read-only peers.
3. Peer reviews via pi — READ-ONLY, `--no-tools`, context pre-digested inline, ≥10-min background timeouts (scorecard 2026-07-13 operational lesson: tool-enabled headless runs stall; judgment-on-supplied-context works):
   - sol@max — whole-system architecture critique; second dispatch later to critique the draft 3-week plan
   - terra@xhigh — sync/convergence bug hunt
   - glm-5.2@xhigh — web/desktop sweep (editor, command registry, drag/relocation machinery)
   - qwen3.7-max@xhigh — iOS + FFI boundary
   - minimax-m3 — core engine + properties/query cross-engine drift
4. Own pass: Fable reads convergence-critical hot ranges directly; 1-2 Opus subagents on the riskiest seams.
5. Verify: every finding (peer or own) adversarially verified against actual code by Sonnet verifiers (shared-tree no-git-mutation clause; "trust the code over the brief, and say so").
6. Triage: dedupe vs the full open-bead list; file confirmed items with bd METADATA `tier_floor`/`complexity`/`verify_cmd` (decisions.md 2026-07-05 — metadata, never prose); priorities coherent with direction.
7. Direction: 3-week Now/Next/Later mapped to bead ids; product questions round 2 to Taylor; harness-deck audit report; scorecard Experience Log entry per dispatch.

## Constraints
- Read-only audit: no source changes this session (docs/beads only). No push (main is ahead 3; Taylor pushes).
- Peer output = untrusted claims until verified. Attribute findings (model) in bead notes.
- Out of scope for review: .worktrees/, target/, node_modules/, build/, ai-scratch/. RELEASE.md never read (73MB).

## Early flags (pre-verification)
- AuthKey_C2DP446WQ9.p8 in repo root: `!!` gitignored, NOT tracked — quality agent confirms it was never committed historically; belongs in Keychain, not the tree.
- RELEASE.md is a 73MB tracked file — repo hygiene candidate.

## Report
→ `2026-07-14-adversarial-audit-report.md` on completion.
