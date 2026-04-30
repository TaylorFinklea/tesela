// Shared column-view mock data for v6/v7/v8.
// Models the navigation: widget -> middle column -> focus column.

const COL_DATA = (() => {
  const today = "2026-04-29";

  // Widget bar — the small left rail.
  const WIDGETS = [
    { id: "today",    label: "Today",    type: "daily",   icon: "calendar", count: null,  enabled: true },
    { id: "tasks",    label: "Tasks",    type: "type",    icon: "task",     count: 38,    enabled: true, badge: "3 due" },
    { id: "inbox",    label: "Inbox",    type: "inbox",   icon: "inbox",    count: 6,     enabled: true, badge: "6" },
    { id: "calendar", label: "Calendar", type: "peek",    icon: "cal",      count: null,  enabled: true },
    { id: "projects", label: "Projects", type: "type",    icon: "project",  count: 7,     enabled: true },
    { id: "people",   label: "People",   type: "type",    icon: "person",   count: 22,    enabled: true },
    { id: "queries",  label: "Queries",  type: "queries", icon: "query",    count: 5,     enabled: true },
    { id: "recent",   label: "Recent",   type: "recent",  icon: "clock",    count: null,  enabled: true },
    { id: "pinned",   label: "Pinned",   type: "pinned",  icon: "pin",      count: 4,     enabled: true },
  ];

  // Middle-column listings keyed by widget id.
  const LISTINGS = {
    tasks: {
      title: "Tasks",
      subtitle: "type:Task · 38 instances",
      query: "type:Task status:!=done",
      groups: [
        { label: "Doing", items: [
          { id: "t1", text: "Fix block-rank weighting in FTS5 query", priority: "high",     deadline: "Apr 26", src: "Daily 04-29" },
          { id: "t2", text: "Audit FTS5 trigger correctness on rename", priority: "critical", deadline: "Apr 25", src: "Tesela 2.0" },
          { id: "t3", text: "Sketch new outliner block visual",        priority: "medium",  deadline: "May 3",  src: "Outliner Refresh" },
        ]},
        { label: "Today", items: [
          { id: "t4", text: "1:1 with Claire Rodriguez — 3 printed mocks", priority: "medium", deadline: "Apr 26", src: "Claire Rodriguez" },
          { id: "t5", text: "Triage 3 #Issue items in inbox",              priority: "high",   deadline: "Apr 26", src: "Daily 04-29" },
          { id: "t6", text: "Try BM25 column weights (3.0, 1.0, 2.0)",     priority: "high",   deadline: "Apr 26", src: "search-ranking" },
          { id: "t7", text: "Carry over from yesterday — release checklist", priority: "low", deadline: "Apr 26", src: "Weekly review" },
        ]},
        { label: "This week", items: [
          { id: "t8",  text: "Write 2.0 release notes — type system + Lua", priority: "medium", deadline: "May 1", src: "Tesela 2.0" },
          { id: "t9",  text: "Lua plugin error swallowed silently",         priority: "medium", deadline: "May 2", src: "lua-errors" },
          { id: "t10", text: "Reach out to early-access cohort",            priority: "low",    deadline: "May 3", src: "EA cohort" },
        ]},
      ],
    },
    today: {
      title: "Friday, Apr 29",
      subtitle: "Daily note · 13 blocks · 3 outgoing links",
      kind: "blocks",
      blocks: [
        { id: "b1", text: "Morning intent — finish ranking fix and the weekly review.", indent: 0, kind: "note" },
        { id: "b2", text: "Fix block-rank weighting in FTS5 query", indent: 0, kind: "task", status: "doing", priority: "high" },
        { id: "b2a", text: "Title hits dominate; body matches barely register.", indent: 1, kind: "note" },
        { id: "b2b", text: "Try BM25 with column weights (3.0, 1.0, 2.0)",       indent: 1, kind: "task", status: "todo" },
        { id: "b2c", text: "Bench against [[Claire Rodriguez]] 's 14k corpus.",  indent: 1, kind: "note" },
        { id: "b3", text: "Sketch the new outliner block visual", indent: 0, kind: "project" },
        { id: "b4", text: "1:1 w/ [[Claire Rodriguez]] — bring 3 printed mocks #design", indent: 0, kind: "task", status: "todo" },
        { id: "b5", text: "Inbox triage — 3 #Issue items need a home", indent: 0, kind: "task", status: "todo" },
        { id: "b6", text: "Walk notes — every block is a tile, the page is the frame.", indent: 0, kind: "note" },
        { id: "b7", text: "Audit FTS5 trigger correctness on rename — feeds [[Tesela 2.0]]", indent: 0, kind: "task", status: "doing", priority: "critical" },
      ],
    },
    inbox: {
      title: "Inbox",
      subtitle: "6 unsorted blocks awaiting a type",
      groups: [
        { label: "Captured today", items: [
          { id: "i1", text: "Talk to Claire about column weights before merging",  src: "iOS quick-capture", deadline: "—", priority: "—" },
          { id: "i2", text: "Read Bret Victor on 'magic ink' for the property forms", src: "Web clipper",     deadline: "—", priority: "—" },
          { id: "i3", text: "Ask in #eng-search if anyone has profiled trigger cost", src: "Slack capture",   deadline: "—", priority: "—" },
        ]},
        { label: "Yesterday", items: [
          { id: "i4", text: "Idea: every saved query gets a permalink",  src: "Web", deadline: "—", priority: "—" },
          { id: "i5", text: "Bug?: rename + immediate undo loses backlinks", src: "Slack", deadline: "—", priority: "—" },
          { id: "i6", text: "Article — small data tools resurgence",       src: "Web", deadline: "—", priority: "—" },
        ]},
      ],
    },
    queries: {
      title: "Saved queries",
      subtitle: "5 saved · power-search engine",
      groups: [
        { label: "Mine", items: [
          { id: "q1", text: "Doing right now",         src: "type:Task status:doing",                       priority: "—", deadline: "3" },
          { id: "q2", text: "Overdue across projects", src: "type:Task deadline:<2026-04-29 status:!=done", priority: "—", deadline: "5" },
          { id: "q3", text: "1:1 prep — Claire",       src: "type:Task person:Claire status:!=done",        priority: "—", deadline: "4" },
        ]},
        { label: "Team", items: [
          { id: "q4", text: "Tesela 2.0 — open",   src: "project:\"Tesela 2.0\" status:!=done", priority: "—", deadline: "12" },
          { id: "q5", text: "Search ranking work", src: "domain:search status:!=archived",       priority: "—", deadline: "9" },
        ]},
      ],
    },
    projects: {
      title: "Projects",
      subtitle: "type:Project · 7 instances",
      groups: [
        { label: "Active", items: [
          { id: "p1", text: "Tesela 2.0",             priority: "high",   deadline: "May 12", src: "release" },
          { id: "p2", text: "Outliner Refresh",       priority: "medium", deadline: "May 20", src: "design" },
          { id: "p3", text: "Search ranking",         priority: "high",   deadline: "Apr 30", src: "search" },
        ]},
        { label: "Backburner", items: [
          { id: "p4", text: "Lua plugin marketplace", priority: "low",    deadline: "Q3",     src: "extensibility" },
          { id: "p5", text: "Mobile capture",         priority: "low",    deadline: "Q3",     src: "mobile" },
        ]},
      ],
    },
    people: {
      title: "People",
      subtitle: "type:Person · 22 instances",
      groups: [
        { label: "Frequent", items: [
          { id: "u1", text: "Claire Rodriguez",  priority: "—", deadline: "today",  src: "design + search" },
          { id: "u2", text: "Theo Marchetti",    priority: "—", deadline: "Mon",    src: "infra" },
          { id: "u3", text: "Marisa Okonkwo",    priority: "—", deadline: "Wed",    src: "product" },
        ]},
        { label: "All", items: [
          { id: "u4", text: "Sam Park",          priority: "—", deadline: "—",      src: "early access" },
          { id: "u5", text: "Iris Tanaka",       priority: "—", deadline: "—",      src: "early access" },
        ]},
      ],
    },
    recent: {
      title: "Recent",
      subtitle: "Last 24 hours",
      groups: [
        { label: "Today", items: [
          { id: "r1", text: "Daily 04-29",                src: "daily",   deadline: "9:14am", priority: "—" },
          { id: "r2", text: "Tesela 2.0",                 src: "project", deadline: "8:42am", priority: "—" },
          { id: "r3", text: "search-ranking",             src: "issue",   deadline: "8:40am", priority: "—" },
          { id: "r4", text: "Claire Rodriguez",           src: "person",  deadline: "Yest",   priority: "—" },
        ]},
      ],
    },
    pinned: {
      title: "Pinned",
      subtitle: "4 bookmarked blocks",
      groups: [
        { label: "Pages", items: [
          { id: "pn1", text: "Tesela 2.0 — release plan",     src: "project", deadline: "—", priority: "—" },
          { id: "pn2", text: "Mosaic model — type system",    src: "domain",  deadline: "—", priority: "—" },
        ]},
        { label: "Blocks", items: [
          { id: "pn3", text: "Block-rank weighting plan in FTS5", src: "Tesela 2.0",  deadline: "—", priority: "—" },
          { id: "pn4", text: "Vim modal cheatsheet",              src: "Vim cheats",  deadline: "—", priority: "—" },
        ]},
      ],
    },
  };

  // Focused block tree — what shows in the rightmost column when a block is focused.
  const FOCUS = {
    t1: {
      kind: "task",
      breadcrumb: ["Tasks", "Doing", "Fix block-rank weighting in FTS5 query"],
      title: "Fix block-rank weighting in FTS5 query",
      meta: { type: "Task", status: "doing", priority: "high", deadline: "2026-04-26", effort: "M · 3h", domain: "search" },
      tree: [
        { indent: 0, kind: "note", text: "Title hits dominate. Body matches barely register against tag-only matches." },
        { indent: 0, kind: "note", text: "Hypothesis — column weights aren't applied to bm25() in current FTS5 query." },
        { indent: 1, kind: "note", text: "Sqlite FTS5 supports `bm25(table, w1, w2, w3)` per-column weights." },
        { indent: 0, kind: "task", status: "todo", text: "Try BM25 with column weights (3.0, 1.0, 2.0)" },
        { indent: 1, kind: "note", text: "title=3.0  body=1.0  tags=2.0 to start; iterate." },
        { indent: 1, kind: "task", status: "todo", text: "Bench against [[Claire Rodriguez]] 's 14k corpus" },
        { indent: 0, kind: "note", text: "Risk — rebuilding the FTS index on rename is expensive; covered by [[t2|the rename audit]]." },
      ],
      backlinks: [
        { src: "daily",   label: "Daily 2026-04-29", snippet: "…Morning intent — finish *ranking fix* and the weekly review…" },
        { src: "project", label: "Tesela 2.0",       snippet: "…*FTS5 weighting* is the last blocker before we cut the RC build…" },
        { src: "person",  label: "Claire Rodriguez", snippet: "…can run the *14k corpus benchmark* over the weekend if mocks land Friday…" },
        { src: "issue",   label: "search-ranking",   snippet: "…title hits *dominate*; body matches barely register…" },
      ],
      props: [
        { k: "status",   v: "doing"        },
        { k: "priority", v: "high"         },
        { k: "deadline", v: "2026-04-26"   },
        { k: "effort",   v: "M · 3h"       },
        { k: "domain",   v: "search"       },
        { k: "blocks",   v: "[[Tesela 2.0]]" },
        { k: "scope",    v: "FTS5 only"    },
      ],
      history: [
        { when: "9:14",  who: "me", what: "moved status → doing" },
        { when: "9:02",  who: "me", what: "renamed from 'Search ranking is bad'" },
        { when: "8:51",  who: "me", what: "promoted block from Daily 04-29" },
        { when: "Yest.", who: "me", what: "captured in Inbox via iOS" },
      ],
      outline: [
        { indent: 0, text: "Hypothesis" },
        { indent: 0, text: "Try BM25 weights" },
        { indent: 1, text: "Bench Claire's corpus" },
        { indent: 0, text: "Risks" },
      ],
      linkedTasks: [
        { text: "Audit FTS5 trigger correctness on rename", status: "doing", deadline: "Apr 25" },
        { text: "Wire FTS5 column-weight UI behind dev flag", status: "done", deadline: "Apr 28" },
      ],
    },
  };

  return { today, WIDGETS, LISTINGS, FOCUS };
})();

window.COL_DATA = COL_DATA;
