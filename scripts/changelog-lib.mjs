export const PLATFORMS = Object.freeze(["web", "desktop", "ios"]);

const PLATFORM_SET = new Set(PLATFORMS);
const TOP_LEVEL_KEYS = new Set(["schemaVersion", "current", "releases"]);
const RELEASE_KEYS = [
  "id",
  "publishedAt",
  "title",
  "summary",
  "platforms",
  "versions",
  "new",
  "fixed",
  "important",
];
const UTC_RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function nonBlankString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function validateString(errors, value, path) {
  if (!nonBlankString(value)) errors.push(`${path} must be a non-empty string`);
}

function validateItems(errors, value, path) {
  if (!Array.isArray(value)) {
    errors.push(`${path} must be an array`);
    return 0;
  }
  value.forEach((item, index) => validateString(errors, item, `${path}[${index}]`));
  return value.length;
}

function validateRelease(errors, release, index) {
  const path = `releases[${index}]`;
  if (!isObject(release)) {
    errors.push(`${path} must be an object`);
    return;
  }
  for (const key of RELEASE_KEYS) {
    if (!(key in release)) errors.push(`${path}.${key} is required`);
  }

  validateString(errors, release.id, `${path}.id`);
  validateString(errors, release.title, `${path}.title`);
  validateString(errors, release.summary, `${path}.summary`);

  if (!nonBlankString(release.publishedAt) || !UTC_RFC3339.test(release.publishedAt)
      || Number.isNaN(Date.parse(release.publishedAt))) {
    errors.push(`${path}.publishedAt must be an RFC 3339 UTC timestamp`);
  }

  if (!Array.isArray(release.platforms) || release.platforms.length === 0) {
    errors.push(`${path}.platforms must be a non-empty array`);
  } else {
    const seen = new Set();
    release.platforms.forEach((platform, platformIndex) => {
      if (!PLATFORM_SET.has(platform)) {
        errors.push(`${path}.platforms[${platformIndex}] is invalid`);
      }
      if (seen.has(platform)) errors.push(`${path}.platforms has duplicate platform ${platform}`);
      seen.add(platform);
    });
  }

  if (!isObject(release.versions)) {
    errors.push(`${path}.versions must be an object`);
  } else {
    if (release.platforms?.includes("desktop")) {
      validateString(errors, release.versions.desktop, `${path}.versions.desktop`);
    }
    if (release.platforms?.includes("ios")) {
      if (!isObject(release.versions.ios)) {
        errors.push(`${path}.versions.ios must be an object`);
      } else {
        validateString(errors, release.versions.ios.marketing, `${path}.versions.ios.marketing`);
        validateString(errors, release.versions.ios.build, `${path}.versions.ios.build`);
      }
    }
  }

  const itemCount = validateItems(errors, release.new, `${path}.new`)
    + validateItems(errors, release.fixed, `${path}.fixed`)
    + validateItems(errors, release.important, `${path}.important`);
  if (itemCount === 0) errors.push(`${path} must contain at least one change item`);
}

function validateArtifact(errors, catalog, artifact) {
  const { platform, version, build } = artifact;
  if (platform === undefined) {
    if (version !== undefined || build !== undefined) {
      errors.push("--version and --build require --platform");
    }
    return;
  }
  if (!PLATFORM_SET.has(platform)) {
    errors.push(`--platform must be one of ${PLATFORMS.join(", ")}`);
    return;
  }

  const release = catalog.releases?.find((entry) => entry?.id === catalog.current?.[platform]);
  if (!release) return;
  if (platform === "web") {
    if (version !== undefined || build !== undefined) {
      errors.push("web releases do not accept --version or --build");
    }
    return;
  }
  if (platform === "desktop") {
    if (!nonBlankString(version)) {
      errors.push("desktop validation requires --version");
    } else if (release.versions?.desktop !== version) {
      errors.push(`desktop version mismatch: catalog ${release.versions?.desktop ?? "missing"}, artifact ${version}`);
    }
    if (build !== undefined) errors.push("desktop validation does not accept --build");
    return;
  }

  if (!nonBlankString(version)) errors.push("iOS validation requires --version");
  if (!nonBlankString(build)) errors.push("iOS validation requires --build");
  const ios = release.versions?.ios;
  if (nonBlankString(version) && ios?.marketing !== version) {
    errors.push(`iOS version mismatch: catalog ${ios?.marketing ?? "missing"}, artifact ${version}`);
  }
  if (nonBlankString(build) && ios?.build !== build) {
    errors.push(`iOS build mismatch: catalog ${ios?.build ?? "missing"}, artifact ${build}`);
  }
}

export function validateCatalog(input, artifact = {}) {
  const errors = [];
  if (!isObject(input)) {
    throw new Error("release-notes catalog must be an object");
  }

  for (const key of TOP_LEVEL_KEYS) {
    if (!(key in input)) errors.push(`top-level field ${key} is required`);
  }
  for (const key of Object.keys(input)) {
    if (!TOP_LEVEL_KEYS.has(key)) errors.push(`top-level field ${key} is not supported`);
  }
  if (input.schemaVersion !== 1) errors.push("schemaVersion must be exactly 1");

  if (!isObject(input.current)) {
    errors.push("current must be an object");
  } else {
    for (const platform of PLATFORMS) {
      validateString(errors, input.current[platform], `current.${platform}`);
    }
    for (const key of Object.keys(input.current)) {
      if (!PLATFORM_SET.has(key)) errors.push(`current.${key} is not a supported platform`);
    }
  }

  if (!Array.isArray(input.releases) || input.releases.length === 0) {
    errors.push("releases must be a non-empty array");
  } else {
    input.releases.forEach((release, index) => validateRelease(errors, release, index));

    const ids = new Set();
    let previousTime = Number.POSITIVE_INFINITY;
    input.releases.forEach((release, index) => {
      if (nonBlankString(release?.id)) {
        if (ids.has(release.id)) errors.push(`duplicate release id ${release.id}`);
        ids.add(release.id);
      }
      const time = Date.parse(release?.publishedAt);
      if (Number.isFinite(time)) {
        if (time >= previousTime) {
          errors.push(`releases must be globally newest-first at releases[${index}].publishedAt`);
        }
        previousTime = time;
      }
    });

    if (isObject(input.current)) {
      for (const platform of PLATFORMS) {
        const id = input.current[platform];
        if (!nonBlankString(id)) continue;
        const target = input.releases.find((release) => release?.id === id);
        if (!target) {
          errors.push(`current.${platform} points to missing release ${id}`);
        } else if (!target.platforms?.includes(platform)) {
          errors.push(`current.${platform} target ${id} does not include platform ${platform}`);
        }
      }
    }
  }

  validateArtifact(errors, input, artifact);
  if (errors.length > 0) {
    throw new Error(`invalid release-notes catalog:\n- ${errors.join("\n- ")}`);
  }
  return input;
}

export function platformHistory(catalog, platform) {
  if (!PLATFORM_SET.has(platform)) throw new Error(`unknown platform ${platform}`);
  const applicable = catalog.releases.filter((release) => release.platforms.includes(platform));
  const currentIndex = applicable.findIndex((release) => release.id === catalog.current[platform]);
  return currentIndex < 0 ? [] : applicable.slice(currentIndex);
}

const SECTIONS = [
  ["New", "NEW", "new"],
  ["Fixed", "FIXED", "fixed"],
  ["Important", "IMPORTANT", "important"],
];

function renderItem(item, prefix) {
  return `${prefix}${item.replaceAll("\n", "\n  ")}`;
}

export function renderRelease(release, format) {
  if (format !== "markdown" && format !== "plain") {
    throw new Error("render format must be markdown or plain");
  }
  const lines = [format === "markdown" ? `# ${release.title}` : release.title, "", release.summary, ""];
  for (const [markdownTitle, plainTitle, key] of SECTIONS) {
    const items = release[key];
    if (items.length === 0) continue;
    lines.push(format === "markdown" ? `## ${markdownTitle}` : plainTitle);
    if (format === "markdown") lines.push("");
    const prefix = format === "markdown" ? "- " : "• ";
    lines.push(...items.map((item) => renderItem(item, prefix)), "");
  }
  return lines.join("\n");
}

export function buildUpdaterManifest({ version, notes, pubDate, target, signature, url }) {
  return `${JSON.stringify({
    version,
    notes,
    pub_date: pubDate,
    platforms: {
      [target]: { signature, url },
    },
  }, null, 2)}\n`;
}
