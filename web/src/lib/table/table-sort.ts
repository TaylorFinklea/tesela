/**
 * tesela-ya4.3 — typed column sort for the generalized query table.
 * Extracted (pure, no Svelte/DOM) so the per-`value_type` comparison rules
 * are unit-testable without mounting `QueryTable.svelte`.
 */

export type SortDirection = "asc" | "desc";

/** Compare two raw property string values per `value_type`. The select /
 *  multi-select branch ranks by declared choice order — the same "low to
 *  high" reading `DisplayChip`'s bar format already implies for those
 *  types — so a sorted select column agrees with how its chips look. */
export function compareTableValues(
  a: string,
  b: string,
  valueType: string,
  choices: string[] | null = null,
): number {
  const av = a ?? "";
  const bv = b ?? "";
  switch (valueType) {
    case "number": {
      const an = parseFloat(av);
      const bn = parseFloat(bv);
      const aValid = av.trim() !== "" && !Number.isNaN(an);
      const bValid = bv.trim() !== "" && !Number.isNaN(bn);
      if (aValid && bValid) return an - bn;
      if (aValid) return -1; // a valid number sorts before an empty/non-numeric value
      if (bValid) return 1;
      return av.localeCompare(bv);
    }
    case "checkbox": {
      const ab = av.trim().toLowerCase() === "true";
      const bb = bv.trim().toLowerCase() === "true";
      if (ab === bb) return 0;
      return ab ? 1 : -1; // unchecked/empty before checked
    }
    case "select":
    case "multi-select": {
      if (choices && choices.length > 0) {
        const rank = (v: string): number => {
          const idx = choices.findIndex((c) => c.toLowerCase() === v.trim().toLowerCase());
          return idx < 0 ? choices.length : idx;
        };
        const ar = rank(av);
        const br = rank(bv);
        if (ar !== br) return ar - br;
      }
      return av.localeCompare(bv);
    }
    default:
      return av.localeCompare(bv);
  }
}

/** Sort `rows` by one column's resolved value, returning a NEW array (the
 *  input is never mutated). `direction === "desc"` reverses the ascending
 *  result rather than negating the comparator, so the empty/invalid-value
 *  placement `compareTableValues` pins (e.g. an empty number sorts last)
 *  moves predictably to the other end instead of needing its own rule per
 *  direction. */
export function sortByColumn<T>(
  rows: T[],
  getValue: (row: T) => string,
  column: { value_type: string; values?: string[] | null },
  direction: SortDirection,
): T[] {
  const sorted = [...rows].sort((a, b) =>
    compareTableValues(getValue(a), getValue(b), column.value_type, column.values ?? null),
  );
  return direction === "asc" ? sorted : sorted.reverse();
}
