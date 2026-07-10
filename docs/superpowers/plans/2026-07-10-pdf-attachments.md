# PDF Attachment Links Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make relative PDF attachment links render as clickable inline chips that open the existing attachment route in a new browser tab, with route and web tests covering the behavior.

**Architecture:** The existing attachment GET route remains the source of PDF bytes and security headers. The CodeMirror unfocused markdown decoration pass will recognize relative PDF links, replace them visually with a `PdfWidget`, resolve the portable source through `resolveImageUrl` in `toDOM`, and call `window.open` from the widget’s click handler. The CodeMirror document remains unchanged, so persistence and focused editing continue to use the original markdown link.

**Tech Stack:** Rust/Axum integration tests, CodeMirror 6 `WidgetType`/decorations, TypeScript, Node test runner, SvelteKit checks.

## Global Constraints

- PDFs open via the existing `GET /attachments/{path}` route in a new browser tab/window.
- Persisted markdown stays unchanged.
- Only relative `attachments/*.pdf` refs are decorated; annotations and iOS behavior are out of scope.
- Preserve the existing strict `default-src 'none'; sandbox` CSP and `nosniff` attachment headers.
- Do not touch `dist/` or push.
- Verify with `cargo test -p tesela-server --test serve_in_process && pnpm --dir web run check && pnpm --dir web run test:unit`.

---

### Task 1: Add the PDF route regression assertion

**Files:**
- Modify: `crates/tesela-server/tests/serve_in_process.rs:90-205`

**Interfaces:**
- Consumes: existing in-process `GET /attachments/{path}` route.
- Produces: an integration assertion that a `.pdf` response is `application/pdf` and retains the attachment security headers.

- [ ] **Step 1: Extend the existing fixture with a PDF file and request**

Add a PDF fixture beside `attachments/icon.png`, request it through the existing `reqwest::Client`, and capture status, content type, CSP, nosniff, and body exactly as the PNG request does. Keep the existing PNG assertions so both image compatibility and PDF behavior are covered.

- [ ] **Step 2: Run the scoped Rust test**

Run: `cargo test -p tesela-server --test serve_in_process`

Expected: PASS if the existing route already maps `.pdf` to `application/pdf`; this is a verification-only test extension for already-shipped route behavior.

- [ ] **Step 3: Assert the PDF contract**

Add assertions for `StatusCode::OK`, `Some("application/pdf")`, `Some("default-src 'none'; sandbox")`, `Some("nosniff")`, and the fixture bytes. Do not alter production route code because `content_type()` and the security headers already implement this contract.

- [ ] **Step 4: Re-run the scoped Rust test**

Run: `cargo test -p tesela-server --test serve_in_process`

Expected: PASS with the PDF assertions included.

---

### Task 2: Add failing web tests for PDF detection and URL resolution

**Files:**
- Modify: `web/tests/unit/cm-decorations.test.mjs:10-145`
- Test target: `web/src/lib/cm-decorations.ts`

**Interfaces:**
- Consumes: exported `isPdfAttachmentRef(src: string): boolean` and existing `resolveImageUrl(src: string, base?: string): string`.
- Produces: executable tests proving only relative attachment PDFs are detected and that their portable source resolves to the attachment route.

- [ ] **Step 1: Write the failing detection test**

Import `isPdfAttachmentRef` and add a test with these expectations:

```js
assert.equal(isPdfAttachmentRef("../attachments/report.pdf"), true);
assert.equal(isPdfAttachmentRef("attachments/report.PDF"), true);
assert.equal(isPdfAttachmentRef("../attachments/report.png"), false);
assert.equal(isPdfAttachmentRef("https://example.com/report.pdf"), false);
```

- [ ] **Step 2: Write the URL-resolution test**

Add a test showing the PDF source uses the existing route resolver without changing the markdown source:

```js
assert.equal(
  resolveImageUrl("../attachments/report.pdf", "/api"),
  "/api/attachments/report.pdf",
);
assert.equal(
  resolveImageUrl("../attachments/report.pdf#page=2", "http://127.0.0.1:7474"),
  "http://127.0.0.1:7474/attachments/report.pdf#page=2",
);
```

- [ ] **Step 3: Run the focused web test to verify RED**

Run: `pnpm --dir web exec node --test tests/unit/cm-decorations.test.mjs`

Expected: FAIL because `isPdfAttachmentRef` is not exported yet; the existing resolver assertion should not be the failure cause.

---

### Task 3: Implement the PDF CodeMirror widget and decoration

**Files:**
- Modify: `web/src/lib/cm-decorations.ts:20-80, 285-315, 930-1070`
- Modify: `web/src/lib/cm-decorations.ts:1449-1480`

**Interfaces:**
- Consumes: `resolveImageUrl`, `MD_LINK_RE`, `findCodeFenceRanges`, existing `WidgetType` and decoration pipeline.
- Produces: exported `isPdfAttachmentRef`; private `PdfWidget` that resolves and opens the route URL; unfocused PDF-link replacement decoration; PDF chip styling.

- [ ] **Step 1: Implement the pure PDF-reference predicate**

Add `isPdfAttachmentRef(src: string): boolean` near `resolveImageUrl`. Trim the source, reject absolute schemes and protocol-relative URLs, normalize backslashes for matching, require an `attachments` path segment, and require the path portion before any `?`/`#` suffix to end in `.pdf` case-insensitively.

- [ ] **Step 2: Implement the minimal `PdfWidget`**

Add a `WidgetType` beside `ImageWidget` with explicit `src` and `label` fields. In `toDOM()`, create an `<a>` element with class `cm-tesela-md-pdf`, set `href` to `resolveImageUrl(this.src)`, `target` to `_blank`, `rel` to `noopener noreferrer`, and accessible text based on the markdown label. Add a click listener that prevents the default navigation and calls `window.open(link.href, "_blank", "noopener,noreferrer")`; return `true` from `ignoreEvent()` so CodeMirror does not focus raw source. `eq()` compares source and label.

- [ ] **Step 3: Add PDF matching to the unfocused markdown pass**

After image matching and before inline code/literal scanning, scan `MD_LINK_RE`. For matches outside code fences that satisfy `isPdfAttachmentRef(m[2])`, add a `Decoration.replace({ widget: new PdfWidget(m[2].trim(), m[1]) })` over the full markdown-link range and record the range. Include the recorded ranges in the `literal()` predicate so later inline markdown passes do not decorate the replaced source a second time. Leave the underlying document untouched.

- [ ] **Step 4: Add chip styling**

Add `.cm-tesela-md-pdf` to the existing markdown decoration theme with inline-flex alignment, the existing primary color, a subtle surface/border treatment, rounded chip corners, pointer cursor, and no unrelated global styles.

- [ ] **Step 5: Run the focused web tests to verify GREEN**

Run: `pnpm --dir web exec node --test tests/unit/cm-decorations.test.mjs`

Expected: PASS, including the new PDF detection/resolution tests and all existing decoration helper tests.

---

### Task 4: Run the full scoped verification and commit

**Files:**
- Verify: `crates/tesela-server/tests/serve_in_process.rs`
- Verify: `web/src/lib/cm-decorations.ts`
- Verify: `web/tests/unit/cm-decorations.test.mjs`
- Include: `docs/superpowers/specs/2026-07-10-pdf-attachments-design.md`
- Include: `docs/superpowers/plans/2026-07-10-pdf-attachments.md`

- [ ] **Step 1: Run the exact requested verification command**

Run:

```bash
cargo test -p tesela-server --test serve_in_process && pnpm --dir web run check && pnpm --dir web run test:unit
```

Expected: all three commands exit 0. The unrelated `sigterm_triggers_validated_backup` failure is outside this scoped command and must not be introduced or investigated here.

- [ ] **Step 2: Inspect the final diff and branch state**

Run: `git diff --check` and `git status --short --branch`.

Expected: no whitespace errors, only the scoped source/tests/design/plan files changed, and branch remains `fleet/attach`; `dist/` is untouched.

- [ ] **Step 3: Commit the complete scoped change**

Run:

```bash
git add crates/tesela-server/tests/serve_in_process.rs web/src/lib/cm-decorations.ts web/tests/unit/cm-decorations.test.mjs docs/superpowers/specs/2026-07-10-pdf-attachments-design.md docs/superpowers/plans/2026-07-10-pdf-attachments.md
git commit -m "feat(web): open PDF attachment links" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

Expected: one conventional commit on `fleet/attach`; do not push.

- [ ] **Step 4: Close the bead**

Run: `bd close tesela-8zd.4 --reason "PDF attachment links now render as route-backed chips, open in new tabs, and are covered by route/web tests."`

Expected: bead reports closed.
