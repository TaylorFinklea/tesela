# Repository Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Leave the repository clean while preserving the intentional iOS build-77 increment in history.

**Architecture:** This is repository hygiene only. The synchronized source and Xcode-project build values remain together in one commit; generated desktop archives are removed without a persistent ignore policy.

**Tech Stack:** Git; XcodeGen project configuration.

## Global Constraints

- Keep `CFBundleVersion` synchronized at `77` in both iOS configuration files.
- Remove only the three untracked files under `dist/desktop/`.
- Do not modify `.gitignore`.
- Do not commit generated release artifacts.

---

### Task 1: Preserve the iOS build increment

**Files:**
- Modify: `app/Tesela-iOS/Info.plist`
- Modify: `app/Tesela-iOS/project.yml`

**Interfaces:**
- Consumes: existing matching `CFBundleVersion` edits from 76 to 77.
- Produces: one commit containing the two synchronized build-number updates.

- [ ] **Step 1: Confirm the two existing edits are identical version increments.**

Run: `git diff -- app/Tesela-iOS/Info.plist app/Tesela-iOS/project.yml`

Expected: both files change only `CFBundleVersion` from `76` to `77`.

- [ ] **Step 2: Commit the synchronized version bump.**

Run: `git add app/Tesela-iOS/Info.plist app/Tesela-iOS/project.yml && git commit -m "chore(ios): bump build number to 77"`

Expected: Git creates a commit containing exactly the two iOS configuration files.

### Task 2: Remove generated desktop artifacts and verify cleanliness

**Files:**
- Delete: `dist/desktop/latest.json`
- Delete: `dist/desktop/Tesela.app.zip`
- Delete: `dist/desktop/Tesela.app.tar.gz`

**Interfaces:**
- Consumes: untracked generated release artifacts.
- Produces: no untracked `dist/` directory and no remaining working-tree changes.

- [ ] **Step 1: Remove only the generated desktop release files.**

Run: `rm dist/desktop/latest.json dist/desktop/Tesela.app.zip dist/desktop/Tesela.app.tar.gz`

Expected: `dist/` contains no files and is no longer reported by Git.

- [ ] **Step 2: Verify the cleanup result excluding the uncommitted plan record.**

Run: `git status --short -- app/Tesela-iOS/Info.plist app/Tesela-iOS/project.yml dist`

Expected: no output.

- [ ] **Step 3: Commit the plan record.**

Run: `git add docs/superpowers/plans/2026-07-11-repository-cleanup.md && git commit -m "docs: add repository cleanup plan"`

Expected: Git creates a documentation-only commit; the working tree remains clean.
