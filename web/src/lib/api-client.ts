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

// Same-origin path; vite dev server proxies `/api/*` → tesela-server at
// 127.0.0.1:7474. Relative URL means the LAN client (phone) hits whatever
// host is serving the page, which avoids exposing the Rust API directly.
const BASE_URL = "/api";

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

async function post<T>(path: string, body: unknown): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new ApiError(res.status, await res.text(), url);
  return (await res.json()) as T;
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
  listNotes: (params: { tag?: string; limit?: number; offset?: number } = {}) => {
    const q = new URLSearchParams();
    if (params.tag) q.set("tag", params.tag);
    if (params.limit !== undefined) q.set("limit", String(params.limit));
    if (params.offset !== undefined) q.set("offset", String(params.offset));
    const qs = q.toString();
    return get<Note[]>(`/notes${qs ? `?${qs}` : ""}`);
  },
  getNote: (id: string) => get<Note>(`/notes/${encodeURIComponent(id)}`),
  updateNote: (id: string, content: string, signal?: AbortSignal) =>
    put<Note>(`/notes/${encodeURIComponent(id)}`, { content }, signal),
  createNote: (title: string, content: string, tags: string[] = []) =>
    post<Note>("/notes", { title, content, tags }),
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
  getType: (name: string) =>
    get<TypeDefinition>(`/types/${encodeURIComponent(name)}`),
  getTypedBlocks: (typeName: string) =>
    get<ParsedBlock[]>(`/types/${encodeURIComponent(typeName)}/blocks`),
  /** Phase 12.2 — fired when status flips to done. Server is responsible
   *  for deciding whether the block actually has a recurring rule. */
  recurBump: (blockId: string) =>
    post<{ bumped: boolean; next_deadline: string | null }>(
      "/blocks/recur-bump",
      { block_id: blockId },
    ),
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
  syncNow: () => post<SyncNowResponse>("/sync/peer/now", {}),
  syncGetPairingCode: () => get<SyncPairingCode>("/sync/peer/pairing-code"),
  syncPairWithCode: (code: string) =>
    post<SyncPairWithCodeResult>("/sync/peer/pair-code", { code }),

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
};

export interface CurrentMosaicResponse {
  path: string;
  config_path: string;
  config_default_mosaic: string | null;
  /** Parent directory the UI uses to suggest new-mosaic paths. */
  suggested_root: string;
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
}
export interface SyncPairWithCodeResult {
  device_id_hex: string;
  display_name: string;
  url: string;
  adopted_group: boolean;
}
