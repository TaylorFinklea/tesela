import type { SearchHit } from "./types/SearchHit";
import type { ParsedBlock } from "./types/ParsedBlock";

export const CONTENT_SEARCH_LIMIT = 20;
export const CONTENT_SEARCH_DEBOUNCE_MS = 180;

export type ContentHitRow = {
  kind: "content";
  key: string;
  noteId: string;
  title: string;
  snippet: string;
  query: string;
  rank: number;
};

export type SnippetRun = { text: string; match: boolean };

export function mapContentHits(hits: SearchHit[], query: string): ContentHitRow[] {
  const normalizedQuery = query.trim();
  return hits
    .filter((hit) => hit.snippet.trim().length > 0)
    .slice(0, CONTENT_SEARCH_LIMIT)
    .map((hit, index) => ({
      kind: "content" as const,
      key: `s:${String(hit.note_id)}:${index}`,
      noteId: String(hit.note_id),
      title: hit.title || String(hit.note_id),
      snippet: hit.snippet,
      query: normalizedQuery,
      rank: hit.rank,
    }));
}

export function snippetRuns(snippet: string): SnippetRun[] {
  const runs: SnippetRun[] = [];
  const marker = /<b>([\s\S]*?)<\/b>/gi;
  let cursor = 0;
  let match: RegExpExecArray | null;
  while ((match = marker.exec(snippet)) !== null) {
    if (match.index > cursor) runs.push({ text: snippet.slice(cursor, match.index), match: false });
    runs.push({ text: match[1], match: true });
    cursor = marker.lastIndex;
  }
  if (cursor < snippet.length) runs.push({ text: snippet.slice(cursor), match: false });
  return runs;
}

function queryTerms(value: string): string[] {
  return value
    .toLocaleLowerCase()
    .replace(/["*()]/g, " ")
    .split(/\s+/)
    .map((term) => term.replace(/^[^\p{L}\p{N}_/-]+|[^\p{L}\p{N}_/-]+$/gu, ""))
    .filter((term) => term.length > 0 && !/^(and|or|not)$/i.test(term));
}

export function findContentBlockId(
  blocks: Pick<ParsedBlock, "id" | "raw_text">[],
  query: string,
  snippet = "",
): string | undefined {
  const terms = queryTerms(query);
  const normalizedBlocks = blocks.map((block) => ({
    id: block.id,
    text: block.raw_text.toLocaleLowerCase(),
  }));

  if (terms.length > 0) {
    const allTerms = normalizedBlocks.find((block) => terms.every((term) => block.text.includes(term)));
    if (allTerms) return allTerms.id;
  }

  const snippetTerms = queryTerms(
    snippetRuns(snippet)
      .filter((run) => run.match)
      .map((run) => run.text)
      .join(" "),
  );
  if (snippetTerms.length > 0) {
    const snippetMatch = normalizedBlocks.find((block) =>
      snippetTerms.every((term) => block.text.includes(term)),
    );
    if (snippetMatch) return snippetMatch.id;
  }

  return normalizedBlocks.find((block) => terms.some((term) => block.text.includes(term)))?.id;
}

export type Debounced<Args extends unknown[]> = ((...args: Args) => void) & {
  cancel: () => void;
};

export function debounce<Args extends unknown[]>(
  fn: (...args: Args) => void,
  delayMs: number,
): Debounced<Args> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const schedule = ((...args: Args) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, delayMs);
  }) as Debounced<Args>;
  schedule.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  return schedule;
}
