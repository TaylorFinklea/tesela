# Non-bullet canonical lift report

**Bead:** `tesela-myh` · **Date:** 2026-07-12 · **Verdict:** implementation and representative product path pass; fresh real-corpus rerun awaits iCloud materialization.

## Delivered

- Fence-aware full-coverage `NoteTree`: headings, prose paragraphs, unindented fences, diagrams, and unclosed fences canonicalize into ordinary blocks.
- Loro hydration/materialization/restart/relay support; stale whole-note identity still fails closed.
- Web and iOS parse/display/edit parity for bid-only fenced blocks.
- One shared stable slug ID and one import writer seam.
- Logseq applies through the active server engine or a separately locked temporary engine; standalone CLI uses the same engine path.
- 16-way bounded import batching keeps each note locked through checked snapshot + Markdown persistence, checkpoints the derived index once, and repairs any index projection drift at boot.
- Partial note failures remain retryable and make CLI automation exit nonzero.

Phase commits: `1f59dd6d`, `83bfe286`, `1d54aa16`, `2a7fa844`, `ba12e0d0`.

## Automated evidence

| Gate | Result |
|---|---|
| `cargo test -p tesela-server -p tesela-sync` | pass; server + sync unit/integration suites, 501-note active-engine route, restart, relay, partial-tail failure |
| `cargo test -p tesela-cli -p tesela-mcp` | pass; CLI lock refusal + nonzero partial-failure result, shared hydration consumers |
| `cargo check -p tesela-sync-ffi` | pass |
| `pnpm --dir web check` | 0 errors; 46 pre-existing warnings |
| `pnpm --dir web test:unit` | 817/817 pass |
| iOS XCTest, iPhone 17 simulator | 503/503 pass |
| independent Lead review | rejected first pass on 2 durability/error bugs + 1 fidelity gap; fixes re-reviewed and approved |

The 501-note integration test verifies every plan item has a durable snapshot and materialized file, and compares the plan's complete structural projection with every materialized note while ignoring only minted bids. Reapply is all `Unchanged`, emits no new relay updates, and restart restores all 501 index entries.

## Product sandbox evidence

Disposable paths (globally ignored):

- graph: `ai-scratch/nonbullet-product-test/logseq`
- mosaic: `ai-scratch/nonbullet-product-test/mosaic`

The representative graph mirrors the exact critical corpus shape from the 2026-07-09 real-graph audit: 19 top-level headings, eight Logseq query blocks, a fenced ASCII diagram containing an internal `- ` line, and two separated prose paragraphs.

| Product check | Observed |
|---|---|
| First CLI import | 4 imported, 0 errors |
| Active-server re-import while flock held | 4 unchanged, 0 writes, success |
| Restart | 4/4 Loro index entries restored |
| Materialized aggregate hash across restart | identical: `696376e3170fcedf5af1cfccf470bcac032199816cee209ab0920591671d8be0` |
| Headings | 19/19 survive |
| Query fences | 8/8 survive |
| Diagram | visible as one code block; internal bullet stays payload |
| Prose | both paragraphs display as separate editable blocks |
| Browser edit | lifted heading edit persisted to Markdown + Loro, survived reload and re-import |
| Cancel path | command palette closes on Escape without navigation |

Headed Playwright QA exercised the actual Graphite `/g` UI: Command-K navigation, TODO dashboard, query code panels, diagram, NixOS prose, edit/blur/save, reload, re-import, and Escape. Four 404 console entries came from pre-existing engine-invisible empty daily files seeded by `tesela init`; this known coverage class is tracked by `tesela-ewj.3` and did not affect imported notes.

Native shell evidence: `pnpm --dir web build` and `cargo build -p tesela-desktop` pass; `tesela-desktop` launches against the sandbox, owns a loopback listener, serves packaged `/g` with HTTP 200, reports the sandbox path with `embedded: true`, and exposes all four Loro index entries. This Codex session did not expose the Computer Use runtime required for a direct native-window screenshot, so the visual interaction evidence is the headed browser run over the same Graphite client.

## Fresh real-graph rerun

The July 9 audit established the real-corpus bar: 19 headings, eight query blocks, the `ai-business` diagram, and NixOS prose. Its old `~/logseq` path is gone. The graph's iCloud container now exists at `~/Library/Mobile Documents/iCloud~com~logseq~logseq/Documents`, but FileProvider hangs indefinitely on `ls`, `find`, `brctl status`, and Finder open. No real graph file was copied or modified in this run.

Once that container materializes (or Taylor supplies a local graph path), rerun the same sandbox import and require: 19/19 headings, 8/8 query fences, both named content cases, identical post-restart materialization, then an all-unchanged re-import. Never target the live mosaic for this check.

## Taylor product check

From the repo root:

```bash
TESELA_MOSAIC="$PWD/ai-scratch/nonbullet-product-test/mosaic" \
TESELA_GROUP_KEY_FILE_STORE=1 TESELA_DISABLE_RELAY=1 \
target/debug/tesela-desktop
```

1. Press `Command-K`, type `TODO`, press `Return`: seven query code panels and the edited `Dashboard one — edited in product test` heading should be visible.
2. Press `Command-K`, open `ai-business`: the full box diagram should be one code block; its `- this line...` payload must not become a separate outliner block.
3. Press `Command-K`, type `NixOS`, press `Return`: five headings and two distinct prose blocks should be visible.
4. Edit any lifted heading/prose, click away, quit, relaunch with the command above: the edit should remain and no block should duplicate.
5. Open Command-K and press `Escape`: the palette should close with the current page unchanged.

The source graph and live mosaic remained read-only. The disposable sandbox is intentionally retained for this named human check.
