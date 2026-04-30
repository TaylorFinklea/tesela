// Shared mock data for v6/v7/v8 column-view variants.
// Tesela treats blocks, pages, types, and queries uniformly — the column model reflects that.

const NOW = "Wed · 2026-04-29 · 10:42";

// Widgets (left bar). Each can be toggled on/off in real app.
const WIDGETS = [
  { id: "today",    label: "Today",         kind: "daily",   icon: "sun",      count: "10:42",   active: false },
  { id: "tasks",    label: "Tasks",         kind: "type",    icon: "check",    count: 12, badge: "doing 3" },
  { id: "inbox",    label: "Inbox",         kind: "query",   icon: "tray",     count: 4 },
  { id: "calendar", label: "Calendar",      kind: "peek",    icon: "calendar", count: "Apr" },
  { id: "projects", label: "Projects",      kind: "type",    icon: "stack",    count: 7 },
  { id: "people",   label: "People",        kind: "type",    icon: "person",   count: 22 },
  { id: "issues",   label: "Issues",        kind: "type",    icon: "bug",      count: 11, badge: "3 new" },
  { id: "rituals",  label: "Rituals",       kind: "type",    icon: "loop",     count: 6 },
  { id: "queries",  label: "Queries",       kind: "section", icon: "lens" },
  { id: "q-stale",  label: "Stale notes",   kind: "query",   icon: "lens",     count: 17, parent: "queries" },
  { id: "q-orphan", label: "Orphans",       kind: "query",   icon: "lens",     count: 9,  parent: "queries" },
  { id: "q-due",    label: "Due this week", kind: "query",   icon: "lens",     count: 5,  parent: "queries" },
  { id: "bookmarks",label: "Bookmarks",     kind: "section", icon: "pin" },
  { id: "b-rel",    label: "Release plan",  kind: "block",   icon: "pin",      parent: "bookmarks" },
  { id: "b-rank",   label: "FTS5 weighting",kind: "block",   icon: "pin",      parent: "bookmarks" },
  { id: "recent",   label: "Recent pages",  kind: "section", icon: "clock" },
  { id: "r-2",      label: "Tesela 2.0",            kind: "page", parent: "recent" },
  { id: "r-1",      label: "Claire Rodriguez",      kind: "page", parent: "recent" },
  { id: "r-3",      label: "search-ranking",        kind: "page", parent: "recent" },
  { id: "r-4",      label: "Outliner refresh",      kind: "page", parent: "recent" },
];

// What's currently in the middle column. Selected by clicking widget "Tasks".
const TASKS_LIST = [
  { id: "t1", title: "Fix block-rank weighting in FTS5 query", type: "task", status: "doing", priority: "high",     due: "Apr 29", source: "search-ranking" },
  { id: "t2", title: "Audit FTS5 trigger correctness on rename", type: "task", status: "doing", priority: "critical", due: "Apr 25", source: "Tesela 2.0", overdue: true },
  { id: "t3", title: "Sketch the new outliner block visual",   type: "task", status: "doing", priority: "medium",   due: "May 3",  source: "Outliner refresh" },
  { id: "t4", title: "1:1 with Claire Rodriguez — bring 3 mocks", type: "task", status: "todo", priority: "medium", due: "Apr 26", source: "Claire Rodriguez" },
  { id: "t5", title: "Triage 3 #Issue items in inbox",          type: "task", status: "todo", priority: "high",    due: "Apr 26", source: "2026-04-29" },
  { id: "t6", title: "Try BM25 column weights (3.0, 1.0, 2.0)", type: "task", status: "todo", priority: "high",    due: "Apr 26", source: "search-ranking" },
  { id: "t7", title: "Carry over unfinished from yesterday",    type: "task", status: "todo", priority: "low",     due: "Apr 26", source: "Weekly review" },
  { id: "t8", title: "Write release notes for 2.0",             type: "task", status: "todo", priority: "medium",  due: "May 1",  source: "Tesela 2.0" },
  { id: "t9", title: "Reach out to early-access cohort",        type: "task", status: "todo", priority: "low",     due: "May 3",  source: "EA cohort" },
  { id: "t10",title: "Wire FTS5 column-weight UI",              type: "task", status: "done", priority: "medium",  due: "Apr 28", source: "Tesela 2.0" },
];

// Right column: focused block (drilled in from t1)
const FOCUSED_BLOCK = {
  id: "t1",
  title: "Fix block-rank weighting in FTS5 query",
  type: "task",
  props: [
    { k: "status",   v: "doing",     swatch: "amber" },
    { k: "priority", v: "high",      swatch: "rose" },
    { k: "due",      v: "Apr 29 · today" },
    { k: "effort",   v: "M · 3h" },
    { k: "domain",   v: "search",    swatch: "sage" },
  ],
  children: [
    { id: "c1", text: "Title hits dominate; body matches barely register in current FTS5 BM25.", type: "note" },
    { id: "c2", text: "Try BM25 with column weights (3.0, 1.0, 2.0)", type: "task", status: "todo" },
    { id: "c3", text: "Bench against [[Claire Rodriguez]]'s 14k corpus before Friday.", type: "note", children: [
      { id: "c3a", text: "Set up baseline run on staging", type: "task", status: "doing" },
      { id: "c3b", text: "Capture top-20 NDCG per query class", type: "note" },
    ]},
    { id: "c4", text: "Confirm trigger correctness on rename — feeds [[Tesela 2.0]] release.", type: "note" },
    { id: "c5", text: "If column weights underperform: fall back to title-prefix boost.", type: "note" },
  ],
};

// Breadcrumb: from a widget down to the focused block.
const CRUMB = [
  { id: "tasks",  label: "Tasks", kind: "type" },
  { id: "doing",  label: "Doing", kind: "group" },
  { id: "t1",     label: "Fix block-rank weighting in FTS5 query", kind: "task" },
];

// Bottom panel — backlinks (default tab)
const BACKLINKS = [
  { src: { type: "daily",   label: "2026-04-29" },        snippet: "Morning intent — finish [[search-ranking|ranking fix]] and the weekly review." },
  { src: { type: "project", label: "Tesela 2.0" },        snippet: "[[fts-weighting|FTS5 weighting]] is the last blocker before we cut the RC build." },
  { src: { type: "person",  label: "Claire Rodriguez" },  snippet: "Can run the 14k corpus benchmark over the weekend if mock numbers land Friday." },
  { src: { type: "issue",   label: "search-ranking" },    snippet: "Title hits dominate; body matches barely register." },
  { src: { type: "ritual",  label: "Weekly review" },     snippet: "Carry over unfinished items into next week." },
];

// Type color tokens (shared)
const TYPE_COLORS = {
  task:    "rose",
  project: "indigo",
  person:  "plum",
  ritual:  "ochre",
  domain:  "sage",
  issue:   "rose",
  note:    "amber",
  daily:   "amber",
  query:   "teal",
  page:    "ink",
};

window.TESELA_DATA = {
  NOW, WIDGETS, TASKS_LIST, FOCUSED_BLOCK, CRUMB, BACKLINKS, TYPE_COLORS,
};
