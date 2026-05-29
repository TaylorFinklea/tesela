/* Shared mock content. Every direction renders the SAME data so the
   comparison is purely about visual treatment, not content. The scene:
   a focused outliner on the "Ship the docs refresh" project, a left
   widget rail (AnyType-style — composable widgets), and a right pane with
   linked references. Keyboard-first throughout. */

const TABS = [
  { id: 'today', name: 'today', kind: 'daily' },
  { id: 'ship', name: 'ship the docs refresh', kind: 'project', active: true },
  { id: 'inbox', name: 'inbox', kind: 'inbox' },
];

// Left rail widgets — the AnyType direction: the rail is a stack of widgets
// the user added; each is a small surface with a header + body.
const WIDGETS = {
  capture: { id: 'capture', title: 'Quick capture', icon: 'bolt' },
  pinned: {
    id: 'pinned', title: 'Pinned', icon: 'pin',
    items: [
      { label: 'Weekly review', kind: 'note', icon: 'fileText' },
      { label: 'Tesela v5 launch', kind: 'project', icon: 'folder' },
      { label: 'Reading list', kind: 'note', icon: 'fileText' },
    ],
  },
  today: {
    id: 'today', title: 'Today', icon: 'sun', badge: 'Apr 10',
    items: [
      { label: 'Standup', meta: '09:30', kind: 'event' },
      { label: 'Docs review w/ Mara', meta: '14:00', kind: 'event' },
      { label: 'Ship docs refresh', meta: 'due', kind: 'task', urgent: true },
    ],
  },
  tasks: {
    id: 'tasks', title: 'Tasks', icon: 'squareCheck', badge: '8',
    groups: [
      { sub: 'Doing', items: [{ label: 'Write getting-started guide', kind: 'task' }] },
      { sub: 'Next', items: [
        { label: 'Fix the login bug', kind: 'task', pri: 'high' },
        { label: 'Audit empty states', kind: 'task' },
      ] },
    ],
  },
};

// The widget order shown in the rail.
const RAIL_ORDER = ['capture', 'pinned', 'today', 'tasks'];

// Focused outliner — block tree for "Ship the docs refresh".
const OUTLINE = [
  {
    id: 'b1', text: 'Ship the docs refresh', tag: 'Task', indent: 0, selected: true,
    props: [
      { k: 'status', v: 'doing', tone: 'doing' },
      { k: 'priority', v: 'high', tone: 'high' },
      { k: 'deadline', v: '2026-04-10', link: true },
    ],
    children: [
      { id: 'b1a', text: 'Write the getting-started guide', indent: 1 },
      { id: 'b1b', text: 'Review with Mara on the Domain team', indent: 1, mention: 'Mara' },
    ],
  },
  {
    id: 'b2', text: 'Fix the login bug', tag: 'Task', tag2: 'urgent', indent: 0,
    props: [{ k: 'status', v: 'todo', tone: 'todo' }],
    children: [
      { id: 'b2a', text: 'Reproduce on staging', indent: 1 },
      { id: 'b2b', text: 'Check the auth middleware', indent: 1 },
    ],
  },
  {
    id: 'b3', text: 'Weekly sync with Domain leads', indent: 0, link: 'Domain',
    props: [],
    children: [],
  },
];

// Right pane — linked references / backlinks buffer.
const BACKLINKS = [
  { src: 'Weekly review', kind: 'note', snippet: 'Blocked on the docs refresh before the v5 cut.' },
  { src: 'Tesela v5 launch', kind: 'project', snippet: 'Docs refresh is a launch gate — owner: self.' },
  { src: '2026-04-08', kind: 'daily', snippet: 'Moved docs refresh to high priority.' },
];

// Property values for the right-pane peek.
const PROPS = [
  { chord: 's', k: 'status', v: 'doing', tone: 'doing' },
  { chord: 'p', k: 'priority', v: 'high', tone: 'high' },
  { chord: 'd', k: 'deadline', v: '2026-04-10' },
  { chord: 'o', k: 'owner', v: 'self' },
];

// Status-line content.
const STATUS = {
  mode: 'NORMAL',
  path: 'today / ship-docs-refresh',
  block: 'b1',
  counts: '23 notes · 8 tasks',
  keys: [
    { k: 'Space', label: 'leader' },
    { k: '⌘K', label: 'command' },
    { k: '⌘\\', label: 'split' },
  ],
};

// Inbox capture items — shared by desktop + mobile inbox screens.
const GR_INBOX = [
  { src: 'microphone', text: 'Remember to add a keyboard cheatsheet to the docs before launch', meta: ['voice · 0:08', '#Task'], sel: true },
  { src: 'bolt', text: 'Idea: inline property editing should support relative dates like "next fri"', meta: ['capture', '#idea'] },
  { src: 'inbox', text: 'Mara: can you review the auth middleware change today?', meta: ['from sync', '@Mara'] },
  { src: 'microphone', text: 'Grocery list for the weekend — oats, coffee, olive oil', meta: ['voice · 0:05'] },
  { src: 'calendar', text: 'Schedule the v5 retro for next week', meta: ['capture', '#Event'] },
];

Object.assign(window, { TABS, WIDGETS, RAIL_ORDER, OUTLINE, BACKLINKS, PROPS, STATUS, GR_INBOX });
