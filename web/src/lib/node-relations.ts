import { scoreFuzzy } from "./fuzzy.ts";

export type PageDirectoryEntry = {
  page_id: string;
  loro_doc_id: string;
  slug: string;
  title: string;
  aliases: string[];
  deleted: boolean;
  forward_to_loro_doc_id?: string | null;
  conflict: boolean;
};

export type ResolvedNode =
  | { state: "resolved"; pageId: string; slug: string; title: string }
  | { state: "deleted" | "unresolved" | "conflict"; pageId: string; label: string };

export function resolveNodeValue(
  pageId: string,
  directory: PageDirectoryEntry[],
): ResolvedNode {
  const records = directory.filter((entry) => entry.page_id === pageId);
  if (records.some((entry) => entry.conflict)) {
    return { state: "conflict", pageId, label: `Conflicting page (${pageId})` };
  }
  const live = records.filter((entry) => !entry.deleted);
  if (live.length === 1) {
    return {
      state: "resolved",
      pageId,
      slug: live[0].slug,
      title: live[0].title || live[0].slug,
    };
  }
  if (live.length > 1) {
    return { state: "conflict", pageId, label: `Conflicting page (${pageId})` };
  }
  if (records.length > 0) {
    return { state: "deleted", pageId, label: `Deleted page (${pageId})` };
  }
  return { state: "unresolved", pageId, label: `Unresolved page (${pageId})` };
}

export function rankPageCandidates(
  directory: PageDirectoryEntry[],
  filter: string,
): PageDirectoryEntry[] {
  const live = directory.filter((entry) => !entry.deleted && !entry.conflict);
  const byPage = new Map<string, PageDirectoryEntry>();
  for (const entry of live) byPage.set(entry.page_id, entry);
  const query = filter.trim();
  return [...byPage.values()]
    .map((entry) => {
      const labels = [entry.title, entry.slug, ...entry.aliases];
      const score = query
        ? Math.max(...labels.map((label) => scoreFuzzy(label, query).score))
        : 1;
      return { entry, score };
    })
    .filter(({ score }) => score > 0)
    .sort((a, b) => b.score - a.score || a.entry.title.localeCompare(b.entry.title))
    .map(({ entry }) => entry);
}
