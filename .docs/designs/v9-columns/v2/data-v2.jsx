// v2 mock data — same shape as v1, slightly enriched
const TODAY_STR = "Friday, April 24 2026";

const NOTES_LIST = [
  { id: "tesela-2.0", title: "Tesela 2.0", type: "Project", letter: "P" },
  { id: "search-ranking", title: "Fix search ranking", type: "Task", letter: "T" },
  { id: "claire-r", title: "Claire Rodriguez", type: "Person", letter: "C" },
  { id: "writing-system", title: "Writing system", type: "Domain", letter: "W" },
  { id: "weekly-review", title: "Weekly review", type: "Ritual", letter: "W" },
  { id: "block-render-bug", title: "Block render flicker", type: "Issue", letter: "B" },
  { id: "morning-pages", title: "Morning pages", type: "Ritual", letter: "M" },
  { id: "type-system-design", title: "Type system design", type: null, letter: "T" },
  { id: "lua-plugin-api", title: "Lua plugin API surface", type: "Project", letter: "L" },
  { id: "outliner-keys", title: "Outliner keymap reference", type: null, letter: "O" },
  { id: "rust-async-notes", title: "Rust async notes", type: "Domain", letter: "R" },
  { id: "release-checklist", title: "Release checklist", type: "Project", letter: "R" },
];

const TYPE_NAV = [
  { name: "Task", count: 38, color: "var(--v2-task)" },
  { name: "Project", count: 11, color: "var(--v2-project)" },
  { name: "Person", count: 7, color: "var(--v2-person)" },
  { name: "Domain", count: 4, color: "var(--v2-domain)" },
  { name: "Ritual", count: 6, color: "var(--v2-ritual)" },
  { name: "Issue", count: 9, color: "var(--v2-issue)" },
];

// Daily blocks. text is parsed for #Tag and [[link]]
const DAILY_BLOCKS = [
  { id: "b1", indent: 0, text: "Morning intent — ship the [[search-ranking|search ranking]] fix and finish weekly review.", meta: "08:14" },
  { id: "b2", indent: 0, type: "task", text: "Fix block-rank weighting in FTS5 query #Task", props: { status: "doing", priority: "high", deadline: "Apr 26" }, meta: "in [[Tesela 2.0]]" },
  { id: "b2a", indent: 1, text: "Looked at sqlite output — title hits dominate, body matches barely register" },
  { id: "b2b", indent: 1, type: "task", text: "Try BM25 with column weights (3.0, 1.0, 2.0) for title/body/tag", props: { status: "todo", priority: "high" } },
  { id: "b2c", indent: 2, text: "Bench against the 14k-note corpus from [[Claire Rodriguez]]" },
  { id: "b3", indent: 0, type: "project", text: "Sketch the new outliner block visual — tile vs. rule treatment #Project", props: { status: "doing", deadline: "May 3" } },
  { id: "b3a", indent: 1, text: "Variants: card with shadow, hairline rule, transparent flow" },
  { id: "b3b", indent: 1, type: "task", text: "1:1 with [[Claire Rodriguez]] on Friday — bring three printed mocks #Task", props: { status: "todo", deadline: "Apr 26", priority: "medium" } },
  { id: "b4", indent: 0, text: "Backlog triage: 3 #Issue items in inbox" },
  { id: "b4a", indent: 1, type: "issue", text: "Block render flicker on indent #Issue", props: { status: "open" } },
  { id: "b4b", indent: 1, type: "issue", text: "Lua plugin error swallowed silently #Issue", props: { status: "thinking" } },
  { id: "b5", indent: 0, text: "Notes from this morning's walk: the mosaic metaphor — every block a tile, the page a frame." },
];

const BACKLINKS = [
  { source: "Tesela 2.0", type: "project", snippet: "Pinned for the week — see <mark>Daily — 2026-04-24</mark> for status." },
  { source: "Weekly review template", type: "ritual", snippet: "Carry over unfinished from <mark>Friday's daily</mark>." },
  { source: "Search ranking", type: "task", snippet: "Plan & bench notes in <mark>today's daily</mark>; see BM25 column weights." },
  { source: "Claire Rodriguez", type: "person", snippet: "Friday 1:1 — three printed mocks discussed in <mark>daily — 2026-04-24</mark>." },
];

const OUTLINE = [
  { d: 0, label: "Morning intent", num: "01" },
  { d: 0, label: "Fix block-rank weighting", num: "02", type: "task" },
  { d: 1, label: "BM25 column weights", num: "02.b", type: "task" },
  { d: 0, label: "Sketch outliner block visual", num: "03", type: "project" },
  { d: 1, label: "1:1 with Claire", num: "03.b", type: "task" },
  { d: 0, label: "Backlog triage", num: "04" },
  { d: 0, label: "Notes from walk", num: "05" },
];

const PAGE_PROPS = [
  { k: "type", v: "Daily" },
  { k: "tags", v: "daily, planning" },
  { k: "created", v: "08:12" },
  { k: "modified", v: "11:43" },
  { k: "linked", v: "4 backlinks" },
  { k: "blocks", v: "12" },
];

const TASK_ROWS = [
  { text: "Fix block-rank weighting in FTS5 query", source: "Daily — 2026-04-24", status: "doing", priority: "high", deadline: "Apr 26" },
  { text: "Try BM25 with column weights (3.0, 1.0, 2.0)", source: "Daily — 2026-04-24", status: "todo", priority: "high", deadline: "Apr 26" },
  { text: "1:1 with Claire Rodriguez — bring 3 printed mocks", source: "Daily — 2026-04-24", status: "todo", priority: "medium", deadline: "Apr 26" },
  { text: "Migrate plugin runtime to wasmtime 21", source: "Lua plugin API surface", status: "backlog", priority: "medium", deadline: "—" },
  { text: "Audit FTS5 trigger correctness on rename", source: "Tesela 2.0", status: "doing", priority: "critical", deadline: "Apr 25" },
  { text: "Write release notes for 0.7", source: "Release checklist", status: "todo", priority: "high", deadline: "Apr 30" },
  { text: "Document the leader-key map", source: "Outliner keymap reference", status: "todo", priority: "low", deadline: "—" },
  { text: "Reproduce the WS reconnect storm", source: "Block render flicker on indent", status: "doing", priority: "high", deadline: "Apr 28" },
];

window.V2Data = { TODAY_STR, NOTES_LIST, TYPE_NAV, DAILY_BLOCKS, BACKLINKS, OUTLINE, PAGE_PROPS, TASK_ROWS };
