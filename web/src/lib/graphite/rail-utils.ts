type RailTaskRow = {
  kind: "task" | "event";
  status: string | null;
  text: string;
};

export const AGENDA_LOOKBACK_DAYS = 90;

function isoDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function agendaRange(now: Date, forwardDays = 60) {
  const from = new Date(now);
  from.setDate(from.getDate() - AGENDA_LOOKBACK_DAYS);
  const to = new Date(now);
  to.setDate(to.getDate() + forwardDays);
  return { from: isoDate(from), to: isoDate(to) };
}

export function agendaQueryKey(from: string, to: string, includeDone = false) {
  return ["agenda", { from, to, includeDone }] as const;
}

export function splitRailTasks<T extends RailTaskRow>(rows: readonly T[]) {
  const openTasks = rows.filter(
    (row) => row.kind === "task" && row.status?.trim().toLowerCase() !== "done",
  );
  const doing = openTasks.filter(
    (row) => row.status?.trim().toLowerCase() === "doing",
  );
  const next = openTasks.filter(
    (row) => row.status?.trim().toLowerCase() !== "doing",
  );

  return { doing, next, total: openTasks.length };
}

export function railTaskLabel(row: Pick<RailTaskRow, "text">): string {
  return row.text.trim() || "(untitled task)";
}
