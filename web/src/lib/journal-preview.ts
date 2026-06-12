/**
 * Pure preview-line extractor for JournalView's OFF-SCREEN section
 * placeholder. Renders a cheap per-block summary (no CodeMirror) so a
 * large imported journal doesn't pay the editor-mount cost for every
 * day. Mirrors the existing off-screen placeholder shape: a stack of
 * truncated one-line rows, no interaction, no editing.
 *
 * The original implementation in JournalView.svelte just split on
 * `\n` and emitted the first six non-empty lines, which surfaced:
 *  - bid-stamp comments (`<!-- bid:UUID -->`)
 *  - property-only continuation lines (`tags::`, `status::`,
 *    `deadline::`, system properties like `apple_reminder_id::`, etc.)
 *  - malformed metadata-looking continuation lines
 *    (e.g. `Deadline::cheduled::`)
 *  - bare empty bullets
 *
 * This helper walks the body in source order, strips the bid stamp,
 * skips continuation lines (any non-bullet line — props are always
 * continuations of the preceding bullet in this format), and yields
 * one preview entry per non-empty bullet block, in order, up to
 * `maxLines` (default 6 — matches the original 6-line cap).
 *
 * IMPORTANT — block text vs continuation. The block-parser's contract
 * is that a `key:: value` form is parsed as a property ONLY on a
 * continuation line (indented, no bullet). The literal text of a
 * bullet line (`- key:: value`) is the user's content and is preserved
 * as-is. This is also the behavior the editor renders, so the preview
 * matches what the user sees when the section mounts.
 *
 * Used only by the unmounted placeholder branch of JournalView; the
 * mounted `BlockOutliner` editing path is untouched (this module is
 * pure and Svelte-free).
 */

// Bid stamp used by the block parser (`web/src/lib/block-parser.ts`).
// Hex UUIDs are 32 chars unhyphenated or 36 chars hyphenated; the parser
// accepts both, so this regex does too. Stripping the stamp before
// bullet detection means a bullet line that is literally just the
// stamp collapses to an empty bullet and is filtered.
const BID_COMMENT_RE = /\s*<!--\s*bid:[0-9a-fA-F-]{32,36}\s*-->/g;

const BULLET_RE = /^(\s*)-\s?(.*)$/;

export interface PreviewLine {
  /** Plain text the user typed on the bullet line (with bid stripped, trimmed). */
  text: string;
  /** Leading-space indent depth; mirrors the original 0/2/4/... structure. */
  indent: number;
}

export interface PreviewOptions {
  /** Maximum number of preview lines to return. Defaults to 6 to match the
   *  previous in-component cap. */
  maxLines?: number;
}

export function previewLines(body: string, opts: PreviewOptions = {}): PreviewLine[] {
  const max = opts.maxLines ?? 6;
  const out: PreviewLine[] = [];
  if (max <= 0) return out;
  for (const rawLine of body.split(/\r?\n/)) {
    if (out.length >= max) break;
    // Strip the bid stamp FIRST so an otherwise-empty bullet
    // (`- <!-- bid:... -->`) collapses to `-` and is filtered below.
    const line = rawLine.replace(BID_COMMENT_RE, "");
    const m = BULLET_RE.exec(line);
    if (!m) continue; // continuation: properties, malformed metadata, blank lines
    const indent = m[1].length;
    const text = m[2].trim();
    if (text === "") continue; // empty bullet (`-` or `- `)
    out.push({ text, indent });
  }
  return out;
}
