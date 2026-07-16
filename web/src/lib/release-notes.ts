import bundledCatalog from "../../../release-notes/releases.json" with { type: "json" };

export type ReleasePlatform = "web" | "desktop" | "ios";

export interface ReleaseNoteVersions {
  desktop?: string;
  ios?: {
    marketing: string;
    build: string;
  };
}

export interface ReleaseNote {
  id: string;
  publishedAt: string;
  title: string;
  summary: string;
  platforms: ReleasePlatform[];
  versions: ReleaseNoteVersions;
  new: string[];
  fixed: string[];
  important: string[];
}

export interface ReleaseCatalog {
  schemaVersion: 1;
  current: Record<ReleasePlatform, string>;
  releases: ReleaseNote[];
}

export interface SeenStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

declare global {
  interface Window {
    __TESELA_PLATFORM__?: string;
  }
}

const PLATFORMS: ReleasePlatform[] = ["web", "desktop", "ios"];
const PLATFORM_SET = new Set<string>(PLATFORMS);
const sessionSeenReleaseIds = new Set<string>();
let bundledWarningLogged = false;

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function nonBlank(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function stringArray(value: unknown, path: string): asserts value is string[] {
  if (!Array.isArray(value)) throw new Error(`${path} must be an array`);
  value.forEach((item, index) => {
    if (!nonBlank(item)) throw new Error(`${path}[${index}] must be a non-empty string`);
  });
}

function parseRelease(value: unknown, index: number): ReleaseNote {
  const path = `releases[${index}]`;
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  for (const key of ["id", "publishedAt", "title", "summary"] as const) {
    if (!nonBlank(value[key])) throw new Error(`${path}.${key} must be a non-empty string`);
  }
  const publishedAt = value.publishedAt;
  if (!nonBlank(publishedAt) || Number.isNaN(Date.parse(publishedAt))) {
    throw new Error(`${path}.publishedAt must be a timestamp`);
  }
  if (!Array.isArray(value.platforms) || value.platforms.length === 0) {
    throw new Error(`${path}.platforms must be a non-empty array`);
  }
  const platformNames = value.platforms as unknown[];
  if (platformNames.some((platform) => !PLATFORM_SET.has(String(platform)))) {
    throw new Error(`${path}.platforms contains an unsupported platform`);
  }
  if (new Set(platformNames).size !== platformNames.length) {
    throw new Error(`${path}.platforms contains a duplicate platform`);
  }
  if (!isRecord(value.versions)) throw new Error(`${path}.versions must be an object`);
  if (platformNames.includes("desktop") && !nonBlank(value.versions.desktop)) {
    throw new Error(`${path}.versions.desktop must be a non-empty string`);
  }
  if (platformNames.includes("ios")) {
    if (!isRecord(value.versions.ios)
        || !nonBlank(value.versions.ios.marketing)
        || !nonBlank(value.versions.ios.build)) {
      throw new Error(`${path}.versions.ios must contain marketing and build`);
    }
  }
  stringArray(value.new, `${path}.new`);
  stringArray(value.fixed, `${path}.fixed`);
  stringArray(value.important, `${path}.important`);
  if (value.new.length + value.fixed.length + value.important.length === 0) {
    throw new Error(`${path} must contain at least one change item`);
  }
  return value as unknown as ReleaseNote;
}

export function parseReleaseCatalog(input: unknown): ReleaseCatalog {
  if (!isRecord(input)) throw new Error("release notes catalog must be an object");
  if (input.schemaVersion !== 1) throw new Error("schemaVersion must be exactly 1");
  if (!isRecord(input.current)) throw new Error("current must be an object");
  if (!Array.isArray(input.releases) || input.releases.length === 0) {
    throw new Error("releases must be a non-empty array");
  }

  const current = input.current;
  for (const platform of PLATFORMS) {
    if (!nonBlank(current[platform])) throw new Error(`current.${platform} must be a release id`);
  }

  const releases = input.releases.map(parseRelease);
  const ids = new Set<string>();
  let priorTime = Number.POSITIVE_INFINITY;
  releases.forEach((release, index) => {
    if (ids.has(release.id)) throw new Error(`duplicate release id ${release.id}`);
    ids.add(release.id);
    const time = Date.parse(release.publishedAt);
    if (time >= priorTime) throw new Error(`releases must be newest-first at index ${index}`);
    priorTime = time;
  });
  for (const platform of PLATFORMS) {
    const id = current[platform] as string;
    const target = releases.find((release) => release.id === id);
    if (!target) throw new Error(`current.${platform} points to missing release ${id}`);
    if (!target.platforms.includes(platform)) {
      throw new Error(`current.${platform} target does not include ${platform}`);
    }
  }

  return {
    schemaVersion: 1,
    current: current as unknown as Record<ReleasePlatform, string>,
    releases,
  };
}

function defaultWarning(message: string) {
  if (bundledWarningLogged) return;
  bundledWarningLogged = true;
  console.warn(message);
}

export function safeReleaseCatalog(
  input: unknown,
  warn: (message: string) => void = defaultWarning,
): ReleaseCatalog | null {
  try {
    return parseReleaseCatalog(input);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    warn(`Release notes unavailable: ${detail}`);
    return null;
  }
}

export function loadBundledReleaseNotes(): ReleaseCatalog | null {
  return safeReleaseCatalog(bundledCatalog);
}

export function resolveReleasePlatform(
  host?: { __TESELA_PLATFORM__?: string },
): "web" | "desktop" {
  const runtimeHost = host ?? (typeof window === "undefined" ? undefined : window);
  return runtimeHost?.__TESELA_PLATFORM__ === "desktop" ? "desktop" : "web";
}

export function platformReleaseHistory(
  catalog: ReleaseCatalog,
  platform: ReleasePlatform,
): ReleaseNote[] {
  const applicable = catalog.releases.filter((release) => release.platforms.includes(platform));
  const currentIndex = applicable.findIndex((release) => release.id === catalog.current[platform]);
  return currentIndex < 0 ? [] : applicable.slice(currentIndex);
}

export function currentRelease(
  catalog: ReleaseCatalog,
  platform: ReleasePlatform,
): ReleaseNote | null {
  return platformReleaseHistory(catalog, platform)[0] ?? null;
}

export function shouldPresentCurrent(
  catalog: ReleaseCatalog,
  platform: ReleasePlatform,
  lastSeen: string | null,
): boolean {
  const applicable = catalog.releases.filter((release) => release.platforms.includes(platform));
  const currentIndex = applicable.findIndex((release) => release.id === catalog.current[platform]);
  if (currentIndex < 0) return false;
  if (!lastSeen) return true;
  const lastSeenIndex = applicable.findIndex((release) => release.id === lastSeen);
  if (lastSeenIndex < 0) return true;
  return lastSeenIndex > currentIndex;
}

export class ReleaseNotesSeenState {
  catalog: ReleaseCatalog;
  platform: ReleasePlatform;
  storage: SeenStorage;
  sessionSeen: Set<string>;

  constructor(
    catalog: ReleaseCatalog,
    platform: ReleasePlatform,
    storage: SeenStorage,
    sessionSeen: Set<string> = sessionSeenReleaseIds,
  ) {
    this.catalog = catalog;
    this.platform = platform;
    this.storage = storage;
    this.sessionSeen = sessionSeen;
  }

  get storageKey(): string {
    return `tesela:releaseNotes:lastSeen:${this.platform}`;
  }

  get currentId(): string {
    return this.catalog.current[this.platform];
  }

  get sessionKey(): string {
    return `${this.platform}:${this.currentId}`;
  }

  shouldAutoPresent(): boolean {
    if (this.sessionSeen.has(this.sessionKey)) return false;
    let lastSeen: string | null = null;
    try {
      lastSeen = this.storage.getItem(this.storageKey);
    } catch {
      lastSeen = null;
    }
    return shouldPresentCurrent(this.catalog, this.platform, lastSeen);
  }

  markCurrentRendered(): void {
    this.sessionSeen.add(this.sessionKey);
    try {
      this.storage.setItem(this.storageKey, this.currentId);
    } catch {
      // Session memory above prevents a render loop when persistence is unavailable.
    }
  }
}

export function releaseVersionLabel(
  release: ReleaseNote,
  platform: ReleasePlatform,
): string {
  if (platform === "desktop") return `Tesela ${release.versions.desktop ?? "Desktop"}`;
  if (platform === "ios") {
    const ios = release.versions.ios;
    return ios ? `Tesela ${ios.marketing} (${ios.build})` : "Tesela for iPhone";
  }
  return "Tesela Web";
}

export function releaseDateLabel(release: ReleaseNote): string {
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(release.publishedAt));
}
