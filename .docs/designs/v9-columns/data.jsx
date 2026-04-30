// Mock data for the prototype

const TODAY_STR = "Friday, April 24 2026";

const NOTES_LIST = [
  { id: "tesela-2.0", title: "Tesela 2.0", tags: ["Project"], badge: "P" },
  { id: "search-ranking", title: "Fix search ranking", tags: ["Task"], badge: "T" },
  { id: "claire-r", title: "Claire Rodriguez", tags: ["Person"], badge: "@" },
  { id: "writing-system", title: "Writing system", tags: ["Domain"], badge: "D" },
  { id: "weekly-review", title: "Weekly review", tags: ["Ritual"], badge: "R" },
  { id: "block-render-bug", title: "Block render flicker on indent", tags: ["Issue"], badge: "I" },
  { id: "morning-pages", title: "Morning pages", tags: ["Ritual"], badge: "R" },
  { id: "type-system-design", title: "Type system design", tags: [] },
  { id: "lua-plugin-api", title: "Lua plugin API surface", tags: ["Project"], badge: "P" },
  { id: "outliner-keys", title: "Outliner keymap reference", tags: [] },
  { id: "rust-async-notes", title: "Rust async notes", tags: ["Domain"], badge: "D" },
  { id: "release-checklist", title: "Release checklist", tags: ["Project"], badge: "P" },
];

const RECENT_IDS = ["search-ranking", "tesela-2.0", "claire-r", "outliner-keys"];

const TAG_NAV = [
  { name: "Task", count: 38, kind: "task" },
  { name: "Project", count: 11, kind: "project" },
  { name: "Person", count: 7, kind: "person" },
  { name: "Domain", count: 4, kind: "domain" },
  { name: "Ritual", count: 6, kind: "domain" },
  { name: "Issue", count: 9, kind: "task" },
];

// Daily-note outliner blocks. Each block: id, indent, text (with #tag and [[wikilinks]] inline), props, children inferred via indent
const DAILY_BLOCKS = [
  {
    id: "b1",
    indent: 0,
    text: 'Morning intent — ship the [[search-ranking|search ranking]] fix and finish the weekly review.',
    props: {},
    meta: "08:14",
  },
  {
    id: "b2",
    indent: 0,
    text: 'Fix block-rank weighting in FTS5 query #Task',
    tag: "task",
    props: { status: "doing", priority: "high", deadline: "Apr 26" },
    meta: "in [[Tesela 2.0]]",
  },
  {
    id: "b2a",
    indent: 1,
    text: 'Looked at sqlite output — title hits dominate, body matches barely register',
    props: {},
  },
  {
    id: "b2b",
    indent: 1,
    text: 'Try BM25 with column weights (3.0, 1.0, 2.0) for title/body/tag',
    props: { status: "todo" },
    tag: "task",
  },
  {
    id: "b2c",
    indent: 2,
    text: 'Bench against the 14k-note corpus from [[Claire Rodriguez]]',
    props: {},
  },
  {
    id: "b3",
    indent: 0,
    text: 'Sketch the new outliner block visual — tile vs. rule treatment #Project',
    tag: "project",
    props: { status: "doing", deadline: "May 3" },
  },
  {
    id: "b3a",
    indent: 1,
    text: 'Variants: card with shadow, hairline rule, transparent flow',
    props: {},
  },
  {
    id: "b3b",
    indent: 1,
    text: '1:1 with [[Claire Rodriguez]] on Friday — bring three printed mocks #Task',
    tag: "task",
    props: { status: "todo", deadline: "Apr 26", priority: "medium" },
  },
  {
    id: "b4",
    indent: 0,
    text: 'Backlog triage: 3 #Issue items in inbox',
    props: {},
  },
  {
    id: "b4a",
    indent: 1,
    text: 'Block render flicker on indent #Issue',
    tag: "task",
    props: { status: "open" },
  },
  {
    id: "b4b",
    indent: 1,
    text: 'Lua plugin error swallowed silently #Issue',
    tag: "task",
    props: { status: "thinking" },
  },
  {
    id: "b5",
    indent: 0,
    text: 'Notes from this morning\'s walk: the mosaic metaphor — every block a tile, the page a frame.',
    props: {},
  },
];

const PAGE_PROPERTIES = [
  { k: "title", v: "Daily — 2026-04-24" },
  { k: "type", v: "Daily" },
  { k: "tags", v: "daily, planning" },
  { k: "created", v: "2026-04-24 08:12" },
  { k: "modified", v: "2026-04-24 11:43" },
];

const BACKLINKS = [
  {
    source: "Tesela 2.0",
    icon: "Project",
    snippet: "Pinned for the week — see <mark>Daily — 2026-04-24</mark> for status.",
  },
  {
    source: "Weekly review template",
    icon: "Ritual",
    snippet: "Carry over unfinished from <mark>Friday's daily</mark>.",
  },
  {
    source: "Search ranking",
    icon: "Task",
    snippet: "Plan & bench notes in <mark>today's daily</mark>; see BM25 column weights.",
  },
  {
    source: "Claire Rodriguez",
    icon: "Person",
    snippet: "Friday 1:1 — three printed mocks discussed in <mark>daily — 2026-04-24</mark>.",
  },
];

const OUTLINE_LINKS = [
  { depth: 0, label: "Morning intent" },
  { depth: 0, label: "Fix block-rank weighting" },
  { depth: 1, label: "Try BM25 column weights" },
  { depth: 0, label: "Sketch outliner block visual" },
  { depth: 1, label: "1:1 with Claire" },
  { depth: 0, label: "Backlog triage" },
  { depth: 0, label: "Notes from walk" },
];

// Tag table — all #Task blocks
const TASK_ROWS = [
  { text: "Fix block-rank weighting in FTS5 query", source: "Daily — 2026-04-24", status: "doing", priority: "high", deadline: "Apr 26", scheduled: "Apr 24" },
  { text: "Try BM25 with column weights (3.0, 1.0, 2.0)", source: "Daily — 2026-04-24", status: "todo", priority: "high", deadline: "Apr 26", scheduled: "—" },
  { text: "1:1 with Claire Rodriguez on Friday — bring 3 printed mocks", source: "Daily — 2026-04-24", status: "todo", priority: "medium", deadline: "Apr 26", scheduled: "Apr 26" },
  { text: "Migrate plugin runtime to wasmtime 21", source: "Lua plugin API surface", status: "backlog", priority: "medium", deadline: "—", scheduled: "—" },
  { text: "Audit FTS5 trigger correctness on rename", source: "Tesela 2.0", status: "doing", priority: "critical", deadline: "Apr 25", scheduled: "Apr 24" },
  { text: "Write release notes for 0.7", source: "Release checklist", status: "todo", priority: "high", deadline: "Apr 30", scheduled: "Apr 28" },
  { text: "Document the leader-key map in /docs", source: "Outliner keymap reference", status: "todo", priority: "low", deadline: "—", scheduled: "—" },
  { text: "Fix sidebar focus when filtering", source: "Tesela 2.0", status: "done", priority: "medium", deadline: "Apr 22", scheduled: "Apr 22" },
  { text: "Replace 'Source Sans' with 'Source Sans 3' globally", source: "Tesela 2.0", status: "done", priority: "low", deadline: "—", scheduled: "Apr 21" },
  { text: "Reproduce the WS reconnect storm", source: "Block render flicker on indent", status: "doing", priority: "high", deadline: "Apr 28", scheduled: "Apr 25" },
  { text: "Add Esc handling to vim-mode autocomplete", source: "Outliner keymap reference", status: "backlog", priority: "low", deadline: "—", scheduled: "—" },
];

window.MockData = {
  TODAY_STR, NOTES_LIST, RECENT_IDS, TAG_NAV,
  DAILY_BLOCKS, PAGE_PROPERTIES, BACKLINKS, OUTLINE_LINKS, TASK_ROWS,
};
