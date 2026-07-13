/**
 * Typed fetch wrapper for tesela-server REST API.
 * Types from ts-rs (crates/tesela-core).
 */
import type { Note } from "$lib/types/Note";
import type { SearchHit } from "$lib/types/SearchHit";
import type { Link } from "$lib/types/Link";
import type { GraphEdge } from "$lib/types/GraphEdge";
import type { TypeDefinition } from "$lib/types/TypeDefinition";
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryResult } from "$lib/types/QueryResult";
import type { CalendarMarks } from "$lib/types/CalendarMarks";
import type { NoteVersion } from "$lib/types/NoteVersion";
import type { AgendaRow } from "$lib/types/AgendaRow";
import type { PropertyDef } from "$lib/types/PropertyDef";
import type { TableColumnConfig } from "$lib/table/table-config";
import type { KeymapConfig } from "$lib/stores/keybindings.svelte";
import { recordLocalSave } from "$lib/ws-refresh-coordinator";
import type { BlockOp } from "$lib/block-ops";
import type { BlockMoveRequest } from "$lib/block-tree-move";
import { buildUpdateNoteBody } from "$lib/api-request-bodies";
import { apiBase } from "$lib/runtime-base";

// `/api` in vite-dev / hosted web (proxy strips `/api` → server root); `""`
// (same-origin root) in the desktop Tauri shell, whose embedded tesela-server
// serves both the API and this UI on one loopback origin. See runtime-base.ts.
// Resolved once at import — the Tauri init script sets the global before this
// bundle evaluates.
const BASE_URL = apiBase();

export class ApiError extends Error {
  constructor(
    public status: number,
    public body: string,
    public url: string,
  ) {
    super(`API ${status} ${url}: ${body}`);
    this.name = "ApiError";
  }
}

async function get<T>(path: string): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, { headers: { Accept: "application/json" } });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as T;
}

/** Like `get`, but also surfaces the `X-Total-Count` header some list
 *  endpoints send (the full match count *before* pagination) — lets a
 *  caller detect a truncated page instead of treating a capped response as
 *  the complete set (tesela-sclr.1). `total` is `null` when the header is
 *  absent. */
async function getWithTotal<T>(path: string): Promise<{ data: T; total: number | null }> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, { headers: { Accept: "application/json" } });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  const totalHeader = res.headers.get("x-total-count");
  const total = totalHeader !== null ? Number(totalHeader) : null;
  return { data: (await res.json()) as T, total };
}

async function post<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
    signal,
  });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as T;
}

async function uploadImage(file: File): Promise<{ path: string; name: string }> {
  const path = `/attachments?filename=${encodeURIComponent(file.name)}`;
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": file.type || "application/octet-stream",
      Accept: "application/json",
    },
    body: file,
  });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as { path: string; name: string };
}

async function put<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, {
    method: "PUT",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
    signal,
  });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as T;
}

export const api = {
  health: () => get<{ status: string }>("/health"),
  uploadImage,
  listNotes: (params: { tag?: string; limit?: number; offset?: number } = {}) => {
    const q = new URLSearchParams();
    if (params.tag) q.set("tag", params.tag);
    if (params.limit !== undefined) q.set("limit", String(params.limit));
    if (params.offset !== undefined) q.set("offset", String(params.offset));
    const qs = q.toString();
    return get<Note[]>(`/notes${qs ? `?${qs}` : ""}`);
  },
  /** Same query as `listNotes`, but also returns the server's total match
   *  count (`X-Total-Count`) so a caller can tell whether its `limit`
   *  truncated the result (tesela-sclr.1). */
  listNotesWithTotal: (params: { tag?: string; limit?: number; offset?: number } = {}) => {
    const q = new URLSearchParams();
    if (params.tag) q.set("tag", params.tag);
    if (params.limit !== undefined) q.set("limit", String(params.limit));
    if (params.offset !== undefined) q.set("offset", String(params.offset));
    const qs = q.toString();
    return getWithTotal<Note[]>(`/notes${qs ? `?${qs}` : ""}`);
  },
  getNote: (id: string) => get<Note>(`/notes/${encodeURIComponent(id)}`),
  /** Whole-note PUT. After the base-diff fix (2026-06-02), the web PUT is
   *  used ONLY for true whole-note writes (frontmatter/title/create + the
   *  remaining loss-avoidance fallbacks). When `baseContent` is supplied it
   *  is the full note body the client last loaded for this note — its edit
   *  BASE — and is sent as `base_content` so the server diffs `base → content`
   *  (the author's REAL changes) instead of `server_file → content`. A block
   *  the author never touched is then identical base→new = NO op = a
   *  concurrent peer edit to it survives. Omitting `baseContent` is backward-
   *  compatible: the server falls back to the historical server-file diff.
   *  See the base-diff spec (2026-06-02) and `UpdateNoteReq.base_content`. */
  updateNote: (
    id: string,
    content: string,
    baseContent?: string,
    signal?: AbortSignal,
  ) => {
    // Open the own-echo suppression window BEFORE the PUT round-trips, so the
    // server's `note_updated` echo for this id is recognised as ours even if
    // it races back ahead of the response. Re-record on the response in case
    // the canonical id differs.
    recordLocalSave(id);
    // Only include `base_content` when a base is supplied — older callers (and
    // true whole-note creates, which have no base) omit it and the server
    // diffs server-file → content as before. Body construction lives in
    // `buildUpdateNoteBody` so the wire shape is unit-tested.
    const reqBody = buildUpdateNoteBody(content, baseContent);
    return put<Note>(`/notes/${encodeURIComponent(id)}`, reqBody, signal).then(
      (note) => {
        recordLocalSave(note.id);
        return note;
      },
    );
  },
  /** Block-granular write (sync redesign 2026-06-02). Submits ONLY the
   *  block ops the user actually changed (in-place text edit, indent/
   *  outdent move) to `POST /notes/{id}/blocks`, instead of PUTting the
   *  whole note body. A block with no op is never re-asserted server-side,
   *  so a concurrent peer edit to it survives — this is the structural fix
   *  for the whole-body clobber (`concurrent_whole_body_clobber.rs`).
   *
   *  Opens the own-echo suppression window BEFORE the POST (mirroring
   *  `updateNote`) so the server's `note_updated` echo for this id is
   *  recognised as ours, then re-records on the response in case the
   *  canonical id differs. Returns the updated Note. */
  upsertBlocks: (noteId: string, ops: BlockOp[], signal?: AbortSignal) => {
    recordLocalSave(noteId);
    return post<Note>(`/notes/${encodeURIComponent(noteId)}/blocks`, { ops }, signal).then(
      (note) => {
        recordLocalSave(note.id);
        return note;
      },
    );
  },
  relocateBlockSubtree: (req: BlockMoveRequest, signal?: AbortSignal) => {
    recordLocalSave(req.source_note_id);
    recordLocalSave(req.destination_note_id);
    return post<{ move_id: string; notes: Note[] }>("/blocks/move-subtree", req, signal).then(
      (result) => {
        for (const note of result.notes) recordLocalSave(note.id);
        return result;
      },
    );
  },
  createNote: (title: string, content: string, tags: string[] = []) => {
    recordLocalSave(title);
    return post<Note>("/notes", { title, content, tags }).then((note) => {
      recordLocalSave(note.id);
      return note;
    });
  },
  getDailyNote: (date?: string) => {
    const q = date ? `?date=${date}` : "";
    return get<Note>(`/notes/daily${q}`);
  },
  search: (query: string, limit = 20) => {
    const q = new URLSearchParams({ q: query, limit: String(limit) });
    return get<SearchHit[]>(`/search?${q.toString()}`);
  },
  executeQuery: (dsl: string, group?: string | null, sort?: string | null) =>
    post<QueryResult>("/search/query", { dsl, group: group ?? null, sort: sort ?? null }),

  // Saved-views registry (spec 2026-06-10). Thin wrappers over the
  // server's `/views` routes (crates/tesela-server/src/routes/views.rs);
  // every write fires a `views_changed` WS event carrying the full
  // ordered registry, so callers can rely on the `["views"]` query being
  // refreshed without an explicit refetch.
  listViews: () => get<ViewRecord[]>("/views"),
  createView: (req: {
    name: string;
    dsl: string;
    display_mode?: string;
    display_group_by?: string;
    display_show_done?: boolean;
    display_table_config?: TableColumnConfig;
  }) => post<ViewRecord>("/views", req),
  updateView: (
    id: string,
    req: {
      name?: string;
      dsl?: string;
      order?: number;
      display_mode?: string;
      display_group_by?: string;
      display_show_done?: boolean;
      /** tesela-ya4.4 — round-trip-authoritative: a saved-view table's
       *  hide/reorder/sort change PUTs the FULL config here every time
       *  (spec decision 4, mirroring `display_group_by`'s write-back). */
      display_table_config?: TableColumnConfig;
    },
  ) => put<ViewRecord>(`/views/${encodeURIComponent(id)}`, req),
  deleteView: (id: string) =>
    fetch(`${BASE_URL}/views/${encodeURIComponent(id)}`, { method: "DELETE" }).then(
      async (r) => {
        if (!r.ok)
          throw new ApiError(r.status, await r.text(), `${BASE_URL}/views/${id}`);
        return (await r.json()) as { deleted: boolean; id: string };
      },
    ),
  /** Body is the bare id array in the new order; every id must exist or
   *  the server rejects the whole reorder. Responds with the re-sorted
   *  registry. */
  reorderViews: (ids: string[]) => post<ViewRecord[]>("/views/reorder", ids),

  /** All property definitions (Property pages) — the views editor uses
   *  the names as DSL key autocomplete candidates. */
  listProperties: () => get<PropertyDef[]>("/properties"),
  getCalendarMarks: (from: string, to: string) => {
    const q = new URLSearchParams({ from, to });
    return get<CalendarMarks>(`/calendar/marks?${q.toString()}`);
  },
  listNoteVersions: (id: string, limit = 50) => {
    const q = new URLSearchParams({ limit: String(limit) });
    return get<NoteVersion[]>(`/notes/${encodeURIComponent(id)}/versions?${q.toString()}`);
  },
  getNoteVersion: (id: string, versionId: number) =>
    get<NoteVersion>(`/notes/${encodeURIComponent(id)}/versions/${versionId}`),
  deleteNote: (id: string) =>
    fetch(`${BASE_URL}/notes/${encodeURIComponent(id)}`, { method: "DELETE" }),
  /** Explicit per-block deletion. Phase 2.2 (sync redesign 2026-05-27):
   *  the server-side PUT diff no longer infers `BlockDelete` from
   *  "absent in PUT body" because clients with stale views were
   *  stomping peer-added blocks they hadn't fetched yet. Web's local
   *  block deletes (dd, backspace-into-empty, backspace-merge) call
   *  this directly so the delete intent is carried explicitly. `bid`
   *  must be the 36-char dashed canonical UUID. */
  deleteBlock: (noteId: string, bid: string) =>
    fetch(
      `${BASE_URL}/notes/${encodeURIComponent(noteId)}/blocks/${encodeURIComponent(bid)}`,
      { method: "DELETE" },
    ),
  getBacklinks: (id: string) =>
    get<Link[]>(`/notes/${encodeURIComponent(id)}/backlinks`),
  getForwardLinks: (id: string) =>
    get<Link[]>(`/notes/${encodeURIComponent(id)}/links`),
  getUnlinkedReferences: (id: string) =>
    get<Link[]>(`/notes/${encodeURIComponent(id)}/unlinked`),
  /** Rename a tag page's slug. Two-phase: with `commit: false` returns the
   *  rewrite counts (refs touched, notes affected) without mutating anything,
   *  so the caller can show a confirmation. With `commit: true` applies the
   *  rewrite (corpus `#tag`/`[[tag]]` rewrite, children's `parent:` rewrite,
   *  and the file move). */
  renameTagSlug: (fromSlug: string, toSlug: string, commit = false) =>
    post<{
      commit: boolean;
      from_slug: string;
      to_slug: string;
      refs: number;
      notes: number;
    }>("/tags/rename", { from_slug: fromSlug, to_slug: toSlug, commit }),
  /** Strip references to a tag from the corpus. Same two-phase contract as
   *  `renameTagSlug`. Used by `:delete-tag` when the user opts into cleanup.
   *  The tag's own page is NOT deleted by this — call `deleteNote(slug)`
   *  after the cleanup completes. */
  cleanupTagReferences: (slug: string, commit = false) =>
    post<{ commit: boolean; slug: string; refs: number; notes: number }>(
      `/tags/${encodeURIComponent(slug)}/cleanup-references`,
      { commit },
    ),
  /** Resolve a path-form tag reference (`nature/birds/cardinal`) or bare
   *  (`cardinal`) into a concrete slug. Missing ancestors are cascade-
   *  created top-down. Returns the resolved leaf slug plus an audit of
   *  newly-created ancestor slugs. */
  resolveTag: (path: string) =>
    post<{ slug: string; cascade_created: string[] }>(
      "/tags/resolve",
      { path },
    ),
  /** Tag usage counts. Phase 15 — surfaced by `:delete-tag` so the user
   *  can see what would be affected before confirming. */
  getTagUsage: (slug: string) =>
    get<{
      slug: string;
      references: number;
      page_instances: number;
      block_instances: number;
      children: number;
    }>(`/tags/${encodeURIComponent(slug)}/usage`),
  getAllEdges: () => get<GraphEdge[]>("/links"),
  listTypes: () => get<TypeDefinition[]>(`/types`),
  getType: (name: string) =>
    get<TypeDefinition>(`/types/${encodeURIComponent(name)}`),
  getTypedBlocks: (typeName: string) =>
    get<ParsedBlock[]>(`/types/${encodeURIComponent(typeName)}/blocks`),
  /** Agenda view — projects recurring tasks forward within [from, to].
   *  Dates are YYYY-MM-DD strings. */
  getAgenda: (from: string, to: string, includeDone = false) =>
    post<AgendaRow[]>("/agenda", { from, to, include_done: includeDone }),

  /** Phase 12.2 — fired when status flips to done. Server is responsible
   *  for deciding whether the block actually has a recurring rule. */
  recurBump: (blockId: string, mode: "complete" | "skip" = "complete") =>
    post<{ bumped: boolean; next_deadline: string | null }>(
      "/blocks/recur-bump",
      { block_id: blockId, mode },
    ),
  /** Agenda interactions — upsert a single `key:: value` property on a block.
   *  The server loads the note, rewrites the property, and saves (triggering
   *  `apply_post_save_bumps` so recurring tasks auto-advance on done).
   *
   *  Block-granular: only this one block's property is rewritten server-side,
   *  so a concurrent peer edit to a sibling block survives (no whole-note PUT).
   *  Opens the own-echo suppression window via `recordLocalSave(noteId)`
   *  BEFORE the POST — matching the block-ops path (`upsertBlocks`) — so the
   *  server's `note_updated` echo for this note isn't mistaken for a remote
   *  change and doesn't trigger a self-clobber refetch. The block_id is
   *  `<note_id>:<line>`, so the note id is the prefix before the last colon. */
  setBlockProperty: (blockId: string, key: string, value: string) => {
    const noteId = blockId.slice(0, blockId.lastIndexOf(":"));
    if (noteId) recordLocalSave(noteId);
    return post<{ ok: boolean }>("/blocks/set-property", {
      block_id: blockId,
      key,
      value,
    });
  },
  /** Clear a single `key:: value` property line from a block. Block-granular
   *  counterpart of `setBlockProperty` for the "unset" case — only this block
   *  is rewritten server-side, so a concurrent peer edit survives. Records the
   *  own-echo suppression window before the POST, same as `setBlockProperty`. */
  clearBlockProperty: (blockId: string, key: string) => {
    const noteId = blockId.slice(0, blockId.lastIndexOf(":"));
    if (noteId) recordLocalSave(noteId);
    return post<{ ok: boolean }>("/blocks/clear-property", {
      block_id: blockId,
      key,
    });
  },
  /** Phase 12.1 — Apple Reminders sync (macOS only). The combined
   *  `remindersSync` is what the "Sync now" UI button hits. */
  remindersPush: () => post<RemindersPushOutcome>("/sync/reminders/push", {}),
  remindersPull: () => post<RemindersPullOutcome>("/sync/reminders/pull", {}),
  remindersSync: () => post<RemindersSyncOutcome>("/sync/reminders", {}),
  remindersStatus: () => get<RemindersLastSync>("/sync/reminders/status"),

  // Phase 2.1 — multi-device peer sync over the LAN
  syncDevice: () => get<SyncDeviceInfo>("/sync/peer/device"),
  syncListPeers: () => get<SyncPeer[]>("/sync/peer/peers"),
  syncAddPeer: (peer: SyncPeer) => post<SyncPeer>("/sync/peer/peers", peer),
  syncRemovePeer: (deviceIdHex: string) =>
    fetch(`${BASE_URL}/sync/peer/peers/${encodeURIComponent(deviceIdHex)}`, {
      method: "DELETE",
    }),
  syncStatus: () => get<SyncPeerStatus[]>("/sync/peer/status"),
  syncDiscovered: () => get<SyncDiscoveredPeer[]>("/sync/peer/discovered"),

  // WAN relay status. Returns `configured: false` when no
  // `[sync.relay]` is set in the mosaic config; otherwise the
  // URL, cursors, last poll/put timestamps, and last error string.
  syncRelayStatus: () => get<RelayStatus>("/sync/relay/status"),
  /** Read the persisted `[sync.relay]` block. Both fields are `null`
   *  when the mosaic is LAN-only. */
  syncRelayGetConfig: () => get<RelayConfigDto>("/sync/relay/config"),
  /** Persist a new `[sync.relay]` block to the mosaic's config.toml.
   *  Takes effect on next server boot — the response carries
   *  `restart_required: true` so the UI can offer a one-click restart. */
  syncRelayPutConfig: (cfg: { url: string; poll_interval_ms: number }) =>
    put<RelayConfigPutResponse>("/sync/relay/config", cfg),
  /** Remove the `[sync.relay]` block (reverts to LAN-only on next boot). */
  syncRelayDeleteConfig: () =>
    fetch(`${BASE_URL}/sync/relay/config`, { method: "DELETE" }).then(async (r) => {
      if (!r.ok) throw new ApiError(r.status, await r.text(), `${BASE_URL}/sync/relay/config`);
      return (await r.json()) as RelayConfigPutResponse;
    }),
  syncGetPairingCode: () => get<SyncPairingCode>("/sync/peer/pairing-code"),
  syncPairWithCode: (code: string) =>
    post<SyncPairWithCodeResult>("/sync/peer/pair-code", { code }),
  /** `tesela-ra7` P0.3c — fetch the current mosaic's 24-word recovery
   *  phrase for the "show recovery phrase" reveal surface. */
  syncRecoveryPhrase: () => get<SyncRecoveryPhrase>("/sync/recovery-phrase"),

  // Phase 13 — backup / export / import
  listBackups: () => get<BackupSummary[]>("/backups"),
  runBackup: (opts: RunBackupRequest) =>
    post<RunBackupResponse>("/backups", opts),
  verifyBackup: (name: string) =>
    post<BackupValidation>(`/backups/${encodeURIComponent(name)}/verify`, {}),
  restoreBackup: (name: string, opts: { in_place?: boolean; allow_newer?: boolean } = {}) =>
    post<BackupRestoreResponse>(`/backups/${encodeURIComponent(name)}/restore`, opts),
  pruneBackups: (dry_run = false) =>
    post<BackupPruneResponse>("/backups/prune", { dry_run }),
  backupKeygen: () => post<{ recipient: string }>("/backups/keygen", {}),
  backupKeyStatus: () => get<BackupKeyStatus>("/backups/key-status"),
  getBackupConfig: () => get<BackupConfigDto>("/backup-config"),
  putBackupConfig: (cfg: BackupConfigDto) => put<BackupConfigDto>("/backup-config", cfg),

  runExport: (opts: { out_path: string; mode: "full" | "portable"; include_attachments?: boolean }) =>
    post<ExportResponse>("/export", opts),
  importObsidian: (source: string, dry_run = false) =>
    post<ImportResponse>("/imports/obsidian", { source, dry_run }),
  importLogseq: (source: string, dry_run = false) =>
    post<ImportResponse>("/imports/logseq", { source, dry_run }),
  importOrg: (source: string, dry_run = false) =>
    post<ImportResponse>("/imports/org", { source, dry_run }),

  /** Phase 13.D follow-up: structured plan+apply for Logseq imports
   *  with per-conflict resolution. The plan carries rendered content
   *  inline so apply doesn't have to re-walk the source. */
  planLogseq: (source: string, mosaic?: string) =>
    post<LogseqPlan>("/imports/logseq/plan", { source, mosaic }),
  applyLogseq: (plan: LogseqPlan, decisions: LogseqDecisions, mosaic?: string) =>
    post<LogseqApplyOutcome>("/imports/logseq/apply", { plan, decisions, mosaic }),

  /** Open a native folder picker on the server's machine (macOS only).
   *  Returns `path: null` when the user cancels. */
  pickFolder: (prompt?: string) =>
    post<{ path: string | null }>("/pick-folder", { prompt: prompt ?? null }),

  // Mosaic management
  currentMosaic: () => get<CurrentMosaicResponse>("/mosaics/current"),
  discoveredMosaics: () => get<DiscoveredMosaic[]>("/mosaics/discovered"),
  createMosaic: (req: CreateMosaicRequest) =>
    post<CreateMosaicResponse>("/mosaics", req),
  switchMosaic: (path: string) =>
    post<{ config_path: string; default_mosaic: string }>("/mosaics/switch", { path }),
  restartServer: () => post<{ respawn_used: boolean }>("/server/restart", {}),

  // Keybinding + leader-tree user config (tesela-cmdd.4). Server-persisted
  // (mirrors backup-config/relay-config's GET/PUT-to-file idiom) so a
  // rebind or leader-tree regroup survives reload on a second device
  // hitting the same tesela-server. The server treats the body as opaque
  // JSON — validation (conflicts, reserved keys) is client-only via
  // `checkRebind`, which needs the live command registry the server
  // doesn't have.
  getKeymapConfig: () => get<KeymapConfig>("/keymap-config"),
  putKeymapConfig: (config: KeymapConfig) =>
    put<KeymapConfig>("/keymap-config", config),
};

/** One saved view in the synced views registry. Mirrors the Rust
 *  `tesela_sync::ViewRecord` (flat display fields, serde snake_case) as
 *  returned by `GET /views` (sorted by `(order, id)`) and carried by the
 *  `views_changed` WS event. Builtins (the seeded Inbox, fixed id
 *  `builtin-inbox`) are editable but never deletable. */
export interface ViewRecord {
  id: string;
  name: string;
  dsl: string;
  order: number;
  builtin: boolean;
  /** "list" | "table" | "kanban". */
  display_mode: string;
  display_group_by: string | null;
  display_show_done: boolean | null;
  /** tesela-ya4.4 — table column display config (hide/reorder/sort). */
  display_table_config: TableColumnConfig | null;
}

export interface CurrentMosaicResponse {
  path: string;
  config_path: string;
  config_default_mosaic: string | null;
  /** Parent directory the UI uses to suggest new-mosaic paths. */
  suggested_root: string;
  /** Running in-process inside the desktop shell — `/server/restart` always
   * 409s there, so the UI disables switch-mosaic controls instead (tesela-ejn.2). */
  embedded: boolean;
}
export interface CreateMosaicRequest {
  /** Custom absolute path. Mutually exclusive with `name`. */
  path?: string;
  /** Bare name; server places at `<data_dir>/tesela/<name>`. */
  name?: string;
  import?: { kind: "obsidian" | "logseq" | "org"; source: string };
}
export interface DiscoveredMosaic {
  name: string;
  path: string;
  is_current: boolean;
  note_count: number;
  last_modified: string | null;
}
export interface CreateMosaicResponse {
  path: string;
  import_stdout: string | null;
  import_stderr: string | null;
  import_success: boolean | null;
}

export interface BackupSummary {
  name: string;
  path: string;
  created_at: string;
  destination_kind: "local" | "external" | "git";
  encryption_kind: "none" | "age";
  file_count: number;
  validated: boolean | null;
  validated_at: string | null;
}
export interface RunBackupRequest {
  destination: "local" | "external" | "git";
  external_path?: string;
  git_remote?: string;
  git_branch?: string;
  encrypt?: boolean;
  no_validate?: boolean;
  no_prune?: boolean;
}
export interface RunBackupResponse {
  path: string;
  file_count: number;
  validated: boolean;
  validation_note: string | null;
}
export interface BackupValidation {
  ok: boolean;
  elapsed_ms: number;
  checked_at: string;
  note: string | null;
}
export interface BackupRestoreResponse {
  target: string;
  renamed_previous: string | null;
  file_count: number;
}
export interface BackupPruneResponse {
  kept: string[];
  removed: string[];
  dry_run: boolean;
}
export interface BackupKeyStatus {
  exists: boolean;
  recipient: string | null;
}
export interface BackupConfigDto {
  auto_on_quit: boolean;
  external_path: string | null;
  git_remote: string | null;
  git_branch: string | null;
}
export interface ExportResponse {
  note_count: number;
  attachment_count: number;
  stripped_property_count: number;
  out_path: string;
}
export interface ImportResponse {
  kind: string;
  success: boolean;
  stdout: string;
  stderr: string;
}

// Logseq plan/apply types — matches tesela_core::import_logseq.
export type LogseqPlanKind =
  | "new_import"
  | "unchanged"
  | "conflict_diff_sha"
  | "conflict_foreign"
  | "hard_skip";

export interface LogseqPlanItem {
  source_rel: string;
  source_sha: string;
  target_id: string;
  target_path: string;
  kind: LogseqPlanKind;
  reason?: string | null;
  rendered_preview?: string | null;
  existing_preview?: string | null;
  existing_sha?: string | null;
  /** Server sends; UI just echoes it back on apply. */
  rendered_full?: string | null;
}
export interface LogseqPlan {
  items: LogseqPlanItem[];
  source: string;
  mosaic: string;
}
export type LogseqDecision =
  | { kind: "skip" }
  | { kind: "overwrite" }
  | { kind: "rename"; suffix: string };
export interface LogseqDecisions {
  per_item: Record<string, LogseqDecision>;
  default: LogseqDecision;
}
export interface LogseqApplyOutcome {
  imported: number;
  overwritten: number;
  renamed: number;
  skipped: number;
  unchanged: number;
  assets_copied: number;
  errors: string[];
}

export interface RemindersPushOutcome {
  created: string[];
  updated: string[];
  synced: string[];
  errors: { block_id: string; message: string }[];
}
export interface RemindersPullOutcome {
  updated: string[];
  orphans: string[];
  errors: { reminder_id: string; message: string }[];
}
export interface RemindersSyncOutcome {
  pull: RemindersPullOutcome;
  push: RemindersPushOutcome;
}
export interface RemindersLastSync {
  at: string | null;
  trigger: string | null;
  outcome: RemindersSyncOutcome | null;
  error: string | null;
}

// Phase 2.1 — peer sync types. Mirror crates/tesela-server/src/routes/peer_sync.rs.
export interface SyncDeviceInfo {
  device_id_hex: string;
}
export interface SyncPeer {
  device_id_hex: string;
  url: string;
  display_name: string | null;
}
export interface SyncPeerStatus {
  device_id_hex: string;
  url: string;
  /** NTP64-encoded HLC of the most recent op we've received from this peer.
   *  Null means we haven't applied anything from them yet. */
  peer_cursor_ntp: number | null;
}
export interface SyncDiscoveredPeer {
  device_id_hex: string;
  display_name: string;
  url: string;
  /** Seconds since the most recent mDNS update from this peer. */
  last_seen_secs_ago: number;
}

/** WAN relay status, mirroring the Rust `RelayStatus` struct in
 *  `tesela-server::sync_relay`. `configured: false` means the
 *  mosaic has no `[sync.relay]` block — every other field is then
 *  zero/null. */
export interface RelayStatus {
  configured: boolean;
  url: string | null;
  /** Highest relay-assigned `seq` we've applied + acked. */
  inbound_cursor: number;
  /** HLC ntp64 of the most-recent local op PUT to the relay. */
  outbound_cursor_ntp: number | null;
  /** Unix seconds — last successful poll. */
  last_poll_at: number | null;
  /** Unix seconds — last successful PUT. */
  last_put_at: number | null;
  /** Unix seconds — when we first registered on this relay. */
  registered_at: number | null;
  /** Most recent error string from poll/put/register, cleared on
   *  next successful tick. `null` when healthy. */
  last_error: string | null;
}

/** Persisted `[sync.relay]` block as returned by GET / accepted by PUT.
 *  Both fields are `null` on GET when the mosaic has no relay block. */
export interface RelayConfigDto {
  url: string | null;
  poll_interval_ms: number | null;
}

/** PUT/DELETE response — echoes what was saved plus a hint that the
 *  caller should restart the server for the change to take effect. */
export interface RelayConfigPutResponse {
  url: string | null;
  poll_interval_ms: number | null;
  /** Always `true` — relay handle is established at boot. */
  restart_required: boolean;
}
/** Per-peer outcome from `POST /sync/peer/now`. Server returns a map keyed
 *  by device_id_hex; each entry has `applied` on success or `error` on
 *  failure. */
export interface SyncNowPeerResult {
  applied?: number;
  error?: string;
}
export interface SyncNowResponse {
  peers: Record<string, SyncNowPeerResult>;
}
/** Phase 2.2 — pairing code we hand to a joining device. */
export interface SyncPairingCode {
  /** Single base64url-no-pad string to copy / paste. Carries the group
   *  id, group key, device id, URL, and display name. */
  code: string;
  display_name: string;
  device_id_hex: string;
  url: string;
  /** Phase 2.5 — 6-character human-typable verifier registered server-side
   *  alongside `code`. The joining device can type this in instead of
   *  scanning the QR; the server resolves it back to `code`. */
  short_code: string;
  /** Seconds the short code remains valid for. The UI can use this to
   *  render a countdown so users know when to regenerate. */
  short_code_expires_in_secs: number;
}
export interface SyncPairWithCodeResult {
  device_id_hex: string;
  display_name: string;
  url: string;
  adopted_group: boolean;
}
/** `tesela-ra7` P0.3c — the current mosaic's 24-word BIP39 recovery
 *  phrase. Space-separated; the phrase IS the group key in plaintext. */
export interface SyncRecoveryPhrase {
  phrase: string;
}
