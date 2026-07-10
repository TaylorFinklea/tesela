# PDF Attachment Links Design

## Goal

Open portable PDF attachment links from the web editor in a new browser tab while keeping persisted markdown unchanged and serving PDFs safely from the existing attachment route.

## Scope

- Detect relative `attachments/*.pdf` markdown link targets in unfocused CodeMirror blocks.
- Render those links as inline PDF chips.
- Resolve the portable attachment path to the existing `GET /attachments/{path}` URL at render time.
- Open the resolved URL in a new tab with `window.open`.
- Verify the route serves PDF bytes with `application/pdf`, strict sandbox CSP, and `nosniff`.
- Exclude annotations and iOS behavior.

## Design

`cm-decorations.ts` will export a pure PDF-reference predicate and reuse `resolveImageUrl` for route URL resolution. During the existing unfocused markdown decoration pass, a matching `[label](relative-pdf-ref)` is replaced visually by a `PdfWidget`. The widget receives the original source and label, resolves the source only in `toDOM`, renders a keyboard-focusable anchor-like chip, and handles click with `window.open` using the resolved route URL. CodeMirror's underlying document remains the original markdown, so focus restores editable source and persistence is unaffected.

PDF matches are excluded from code fences and other literal inline regions. Existing generic markdown links, wiki-links, images, and external URL safety rules remain unchanged.

The existing in-process attachment integration test will add a PDF fixture/request alongside the PNG request and assert the PDF content type plus the already-required sandbox CSP and `nosniff` headers.

## Acceptance

- `[Report](../attachments/report.pdf)` displays as an inline PDF chip when unfocused.
- Clicking the chip calls `window.open` with the resolved `/attachments/report.pdf` URL, `_blank`, and safe opener flags.
- The markdown source remains unchanged in the editor document.
- Non-PDF links, external links, images, and fenced-code examples retain current behavior.
- The scoped Rust and web verification command passes, apart from the documented unrelated `tesela-64g` failure outside the scoped command.
