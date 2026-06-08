/**
 * Task priority — p1/p2/p3/p4 flags (Model B, decided 2026-06-08).
 *
 * `priorityFlag` renders P1–P3 as colored flags (P1 red / P2 amber / P3 blue);
 * P4 (and the legacy "low") is the default and shows NO flag, per the design.
 * `priorityLevel` normalizes BOTH the new p1..p4 values AND legacy
 * critical/high/medium/low so existing `priority::` data still maps to a flag
 * — no destructive data migration needed.
 */

export type PriorityFlag = {
  level: 1 | 2 | 3;
  /** "P1" / "P2" / "P3". */
  label: string;
  color: string;
  bg: string;
};

const FLAGS: Record<1 | 2 | 3, { color: string; bg: string }> = {
  1: { color: "#EB5C58", bg: "rgba(235, 92, 88, 0.15)" }, // red — urgent
  2: { color: "#E8A33D", bg: "rgba(232, 163, 61, 0.15)" }, // amber — high
  3: { color: "#6B9AE0", bg: "rgba(107, 154, 224, 0.15)" }, // blue — medium
};

/** Normalize any priority value → 1..4 (4 = low/default), or null if unset. */
export function priorityLevel(value: string | undefined | null): 1 | 2 | 3 | 4 | null {
  const v = (value ?? "").trim().toLowerCase();
  if (!v) return null;
  if (v === "p1" || v === "critical" || v === "urgent") return 1;
  if (v === "p2" || v === "high") return 2;
  if (v === "p3" || v === "medium" || v === "med") return 3;
  if (v === "p4" || v === "low" || v === "none") return 4;
  return null;
}

/** A renderable flag for P1–P3. P4 / low / unset → null (no flag, by design). */
export function priorityFlag(value: string | undefined | null): PriorityFlag | null {
  const level = priorityLevel(value);
  if (level == null || level === 4) return null;
  return { level, label: `P${level}`, ...FLAGS[level] };
}

/** Stored values for the click-to-cycle (P1 → P2 → P3 → P1). */
export const PRIORITY_CYCLE = ["p1", "p2", "p3"] as const;
