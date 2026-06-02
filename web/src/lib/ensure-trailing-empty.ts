// Pure decision/builder helpers for JournalView's `ensureTrailingEmpty`.
//
// Extracted from the `.svelte` component so the position-aware scan and the
// bid-stamped emit are unit-testable (the component wrapper keeps the async
// `api.getNote` / `api.updateNote` I/O). See
// `web/tests/unit/ensure-trailing-empty.test.mjs`.

const BID_MARKER = /<!--\s*bid:[^>]*-->\s*$/;
// A "trailing-style empty bullet": a bullet line with no text after the dash
// (`-` or `- ` plus optional indentation), once any trailing bid marker is
// stripped.
const EMPTY_BULLET = /^\s*-\s*$/;

/** Strip a trailing `<!-- bid:UUID -->` marker from a single line. */
function stripTrailingBid(line: string): string {
  return line.replace(BID_MARKER, "").trimEnd();
}

/**
 * True when the body ALREADY contains a focusable trailing-style empty bullet
 * anywhere (last line OR stranded mid-body). Position-aware: the engine can
 * append a fresh end node after a previously-trailing empty, stranding the
 * empty mid-body so a last-line-only check would miss it and accrete a new
 * one on every mount. Any existing empty bullet means the user already has a
 * focusable line — do NOT append another.
 */
export function bodyHasTrailingEmpty(body: string): boolean {
  for (const line of body.split("\n")) {
    if (EMPTY_BULLET.test(stripTrailingBid(line))) return true;
  }
  return false;
}

/**
 * Build the new note content with exactly one bid-stamped empty bullet
 * appended after `body`. The bid stamp stops the server re-minting a fresh
 * UUID + re-appending a new end node on every mount: on the NEXT mount the
 * stamped empty bid-strips back to `- ` and `bodyHasTrailingEmpty` matches,
 * so we return early instead of appending again.
 *
 * `content` is the full note (front-matter + body); `body` is the already-
 * `\n`-trimmed body the empty is appended to. `newBid` is injected for
 * deterministic tests (defaults to `crypto.randomUUID()`).
 */
export function appendTrailingEmpty(
  content: string,
  body: string,
  newBid: string = crypto.randomUUID(),
): string {
  const emptyLine = `- <!-- bid:${newBid} -->`;
  const newBody = (body.length > 0 ? body + "\n" : "") + emptyLine + "\n";
  const fmEnd = content.startsWith("---") ? content.indexOf("---", 3) : -1;
  const splitAt = fmEnd >= 0 ? fmEnd + 3 + (content[fmEnd + 3] === "\n" ? 1 : 0) : 0;
  return content.slice(0, splitAt) + newBody;
}
