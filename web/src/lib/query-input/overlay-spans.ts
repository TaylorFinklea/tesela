/**
 * Builds the span list QueryInput's colored underlay renders (tesela-vp9.2,
 * spec decision 1's "overlay technique" — a transparent-text input stacked
 * over a colored-span underlay produced from `tokenize()` output). Pure:
 * takes the current source string plus an optional diagnostics list (from
 * `parseQueryWithDiagnostics`, debounced by the caller) and returns spans
 * covering the ENTIRE string (including whitespace gaps) so the underlay's
 * text content matches the real input glyph-for-glyph.
 */
import { tokenize, type Diagnostic } from "../query-language.ts";
import { classifyTokens, type TokenRole } from "./classify.ts";

export type OverlaySpan = {
  start: number;
  end: number;
  text: string;
  /** "text" = a whitespace gap between tokens (or the whole string, when
   *  input is empty/all-whitespace) — renders with no role color. */
  role: TokenRole | "text";
  /** True when this span overlaps a diagnostic — the underlay underlines it. */
  diagnostic: boolean;
};

function overlapsAny(start: number, end: number, diagnostics: readonly Diagnostic[]): boolean {
  return diagnostics.some((d) => start < d.end && end > d.start);
}

export function buildOverlaySpans(
  input: string,
  diagnostics: readonly Diagnostic[] = [],
): OverlaySpan[] {
  const tokens = tokenize(input);
  const classified = classifyTokens(tokens);
  const spans: OverlaySpan[] = [];
  let cursor = 0;

  const push = (start: number, end: number, role: OverlaySpan["role"]) => {
    spans.push({
      start,
      end,
      text: input.slice(start, end),
      role,
      diagnostic: overlapsAny(start, end, diagnostics),
    });
  };

  for (const c of classified) {
    if (c.span.start > cursor) push(cursor, c.span.start, "text");
    push(c.span.start, c.span.end, c.role);
    cursor = c.span.end;
  }
  if (cursor < input.length) push(cursor, input.length, "text");

  return spans;
}
