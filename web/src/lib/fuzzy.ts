/**
 * Phase 9.8 — small fuzzy scorer for inline pickers (`[[`, `#`, etc.).
 *
 * Tiered scoring so prefix matches always beat substrings, substrings always
 * beat subsequences, and ties resolve by match position. The caller tie-breaks
 * across same-score items (typically by recency).
 *
 *   Prefix match     → 1000 + (tighter prefix wins)
 *   Word-start match → 500 - position
 *   Substring match  → 200 - position
 *   Subsequence      → max(1, 50 - span)
 *   No match         → 0
 *
 * Returns the matched character positions (in the original label, not the
 * lowercased copy used for matching) so the caller can render highlights.
 */

export type FuzzyMatch = {
  score: number;
  /** Indices into the original `label` where filter chars matched. */
  positions: number[];
};

export type StrongFuzzyMatch = FuzzyMatch & {
  label: string;
};

const STRONG_NEAR_MATCH_SCORE = 40;
const STRONG_NEAR_MATCH_MIN_INPUT = 3;

export function findStrongFuzzyMatch(
  input: string,
  labels: Iterable<string>,
): StrongFuzzyMatch | null {
  const filter = input.trim();
  if (filter.length < STRONG_NEAR_MATCH_MIN_INPUT) return null;

  const exact = filter.toLowerCase();
  let best: StrongFuzzyMatch | null = null;
  for (const label of labels) {
    const candidate = label.trim();
    if (!candidate || candidate.toLowerCase() === exact) return null;
    const match = scoreFuzzy(candidate, filter);
    if (match.score < STRONG_NEAR_MATCH_SCORE) continue;
    if (!best || match.score > best.score || (match.score === best.score && candidate.length < best.label.length)) {
      best = { label: candidate, score: match.score, positions: match.positions };
    }
  }
  return best;
}

export function scoreFuzzy(label: string, filter: string): FuzzyMatch {
  if (!filter) return { score: 0, positions: [] };
  const llabel = label.toLowerCase();
  const lfilter = filter.toLowerCase();

  // Prefix.
  if (llabel.startsWith(lfilter)) {
    const positions = Array.from({ length: lfilter.length }, (_, i) => i);
    return { score: 1000 + (label.length === filter.length ? 50 : 0), positions };
  }

  // Substring.
  const sIdx = llabel.indexOf(lfilter);
  if (sIdx !== -1) {
    const positions = Array.from({ length: lfilter.length }, (_, i) => sIdx + i);
    const wordStart = sIdx === 0 || /[\s_/-]/.test(label[sIdx - 1] ?? "");
    return {
      score: (wordStart ? 500 : 200) - sIdx,
      positions,
    };
  }

  // Subsequence — chars in order, possibly with gaps.
  const positions: number[] = [];
  let li = 0;
  for (let fi = 0; fi < lfilter.length; fi++) {
    while (li < llabel.length && llabel[li] !== lfilter[fi]) li++;
    if (li >= llabel.length) return { score: 0, positions: [] };
    positions.push(li);
    li++;
  }
  const span = positions[positions.length - 1] - positions[0];
  return { score: Math.max(1, 50 - span), positions };
}

/** Split a label into runs of `{ ch, match }` for highlighted rendering. */
export function highlightRuns(
  label: string,
  positions: number[],
): { ch: string; match: boolean }[] {
  const set = new Set(positions);
  return Array.from(label, (ch, i) => ({ ch, match: set.has(i) }));
}
