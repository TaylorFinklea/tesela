/**
 * Renderer registration entry point.
 *
 * Importing this module (with side-effects) registers every renderer with
 * its respective registry. Per the v5 spec, registration is explicit — no
 * filesystem-discovery magic; the imports are greppable and HMR-safe.
 *
 * Phase 4: derived renderers.
 * Phase 5: ambient renderers.
 * Phase 4+: page renderers (currently still dispatched by NoteRenderer
 * inside BufferShell — the page registry is reserved for the wholesale
 * page-type registry refactor planned alongside renderer cascades).
 */

import { register as registerDerived } from "./derived/index.ts";
import { register as registerAmbient } from "./ambient/index.ts";

import backlinksOfPage from "./derived/backlinks-of-page";
import outlineOfPage from "./derived/outline-of-page";
import propertiesOfPage from "./derived/properties-of-page";
import tasksLinkedToPage from "./derived/tasks-linked-to-page";
import localGraphOfPage from "./derived/local-graph-of-page";
import instancesOfTag from "./derived/instances-of-tag";
import backlinksOfTag from "./derived/backlinks-of-tag";

import calendar from "../ambients/calendar";
import todayInProgress from "../ambients/today-in-progress";
import workspaceDashboard from "../ambients/workspace-dashboard";
import aiWorkspace from "../ambients/ai-workspace";
import agenda from "../ambients/agenda";
import inbox from "../ambients/inbox";

registerDerived("backlinks-of-page", backlinksOfPage);
registerDerived("outline-of-page", outlineOfPage);
registerDerived("properties-of-page", propertiesOfPage);
registerDerived("tasks-linked-to-page", tasksLinkedToPage);
registerDerived("local-graph-of-page", localGraphOfPage);
registerDerived("instances-of-tag", instancesOfTag);
registerDerived("backlinks-of-tag", backlinksOfTag);

registerAmbient("calendar", calendar);
registerAmbient("today-in-progress", todayInProgress);
registerAmbient("workspace-dashboard", workspaceDashboard);
registerAmbient("ai-workspace", aiWorkspace);
registerAmbient("agenda", agenda);
registerAmbient("inbox", inbox);
