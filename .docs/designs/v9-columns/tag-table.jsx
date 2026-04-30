// Tag table — query view for typed blocks
const { useState: useTagState } = React;

function TagTable() {
  const I = window.Icons;
  const D = window.MockData;
  const [statusFilter, setStatusFilter] = useTagState("all");
  const [priorityFilter, setPriorityFilter] = useTagState("all");

  let rows = D.TASK_ROWS;
  if (statusFilter !== "all") rows = rows.filter((r) => r.status === statusFilter);
  if (priorityFilter !== "all") rows = rows.filter((r) => r.priority === priorityFilter);

  const filters = [
    { key: "status", value: "all", label: "All status" },
    { key: "status", value: "doing", label: "Doing" },
    { key: "status", value: "todo", label: "Todo" },
    { key: "status", value: "done", label: "Done" },
    { key: "status", value: "backlog", label: "Backlog" },
  ];
  const priorityFilters = [
    { value: "all", label: "Any priority" },
    { value: "critical", label: "Critical" },
    { value: "high", label: "High" },
    { value: "medium", label: "Medium" },
    { value: "low", label: "Low" },
  ];

  return (
    <div className="tag-table-wrap">
      <div className="tag-table-head">
        <span style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--muted-foreground)", textTransform: "uppercase", letterSpacing: "0.14em" }}>
          #
        </span>
        <h1>Task</h1>
        <span className="count">{rows.length} blocks across {new Set(rows.map(r => r.source)).size} pages</span>
        <span style={{ flex: 1 }} />
        <button className="icon-btn" style={{ width: 32, height: 32 }} title="New query">
          <I.Plus />
        </button>
      </div>

      <div className="tag-filters">
        <span style={{ display: "inline-flex", alignItems: "center", gap: 6, padding: "5px 8px", color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 11 }}>
          <I.Filter size={12} /> filter
        </span>
        {filters.map((f) => (
          <span
            key={f.value}
            className={`tag-filter ${statusFilter === f.value ? "active" : ""}`}
            onClick={() => setStatusFilter(f.value)}
          >
            <span className="k">status::</span>
            {f.label}
          </span>
        ))}
        <span style={{ width: 12 }} />
        {priorityFilters.map((f) => (
          <span
            key={f.value}
            className={`tag-filter ${priorityFilter === f.value ? "active" : ""}`}
            onClick={() => setPriorityFilter(f.value)}
          >
            <span className="k">priority::</span>
            {f.label}
          </span>
        ))}
      </div>

      <table className="tag-table">
        <thead>
          <tr>
            <th className="col-text">Block <span className="sort-ico">↓</span></th>
            <th>Status</th>
            <th>Priority</th>
            <th>Deadline</th>
            <th>Scheduled</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td className="col-text">
                <div className="tt-text">{r.text}</div>
                <div className="tt-source">
                  <I.File size={11} className="ico" /> {r.source}
                </div>
              </td>
              <td>
                <span className={`prop-chip status-${r.status}`}>
                  <span className="v">{r.status}</span>
                </span>
              </td>
              <td>
                <span className={`prop-chip priority-${r.priority}`}>
                  <span className="v">{r.priority}</span>
                </span>
              </td>
              <td style={{ fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--foreground-muted)" }}>{r.deadline}</td>
              <td style={{ fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--foreground-muted)" }}>{r.scheduled}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <div style={{ marginTop: 16, display: "flex", gap: 16, fontSize: 11.5, color: "var(--muted-foreground)", fontFamily: "var(--font-mono)" }}>
        <span>query: <span style={{ color: "var(--primary)" }}>tag = #Task</span></span>
        <span>group: source</span>
        <span>sort: deadline ↑</span>
        <span style={{ marginLeft: "auto" }}>
          press <span className="kbd">e</span> to edit query
        </span>
      </div>
    </div>
  );
}

window.TagTable = TagTable;
