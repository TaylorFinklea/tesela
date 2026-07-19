export type MultiSelectDelta = {
  current: string[];
  add: string[];
  remove: string[];
};

function stableUnique(values: Iterable<string>): string[] {
  const result: string[] = [];
  for (const raw of values) {
    const value = raw.trim();
    if (value && !result.includes(value)) result.push(value);
  }
  return result;
}

/** Decode the canonical Markdown projection of a LoroList for display only. */
export function parseMultiSelectValue(value: string): string[] {
  return stableUnique(value.split(","));
}

export function isMultiSelectType(valueType: string): boolean {
  return valueType === "multi-select" || valueType === "multiselect";
}

/** Compute independent CRDT member operations; never emits a replacement string. */
export function multiSelectDelta(currentValue: string, selected: Iterable<string>): MultiSelectDelta {
  const current = parseMultiSelectValue(currentValue);
  const next = stableUnique(selected);
  return {
    current,
    add: next.filter((item) => !current.includes(item)),
    remove: current.filter((item) => !next.includes(item)),
  };
}

export function applyMultiSelectDelta(
  currentValue: string,
  add: Iterable<string>,
  remove: Iterable<string>,
): string {
  const removed = new Set(stableUnique(remove));
  const next = parseMultiSelectValue(currentValue).filter((item) => !removed.has(item));
  for (const item of stableUnique(add)) {
    if (!next.includes(item)) next.push(item);
  }
  return next.join(", ");
}

export function checkboxIsChecked(value: string): boolean {
  return value.trim().toLowerCase() === "true";
}

export function toggledCheckboxValue(value: string): "true" | "false" {
  return checkboxIsChecked(value) ? "false" : "true";
}

export function propertyLinkTarget(valueType: string, value: string): string | null {
  const raw = value.trim();
  if (!raw) return null;

  switch (valueType) {
    case "url": {
      if (/^https?:\/\//i.test(raw)) return raw;
      if (/^\/\//.test(raw)) return `https:${raw}`;
      if (/^[a-z][a-z0-9+.-]*:/i.test(raw)) return null;
      return `https://${raw}`;
    }
    case "email": {
      const address = raw.replace(/^mailto:/i, "");
      if (!address.includes("@") || /\s/.test(address)) return null;
      return `mailto:${address}`;
    }
    case "phone": {
      const number = raw.replace(/^tel:/i, "").replace(/[^+0-9*#,;]/g, "");
      return /\d/.test(number) ? `tel:${number}` : null;
    }
    default:
      return null;
  }
}
