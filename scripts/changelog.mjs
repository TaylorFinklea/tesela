#!/usr/bin/env node

import { readFile } from "node:fs/promises";

import { buildUpdaterManifest, renderRelease, validateCatalog } from "./changelog-lib.mjs";

const CATALOG_URL = new URL("../release-notes/releases.json", import.meta.url);

function usage() {
  return [
    "Usage:",
    "  node scripts/changelog.mjs validate [--platform web|desktop|ios] [--version V] [--build N]",
    "  node scripts/changelog.mjs render --release ID --format markdown|plain",
    "  node scripts/changelog.mjs updater-manifest --version V --notes-file PATH --pub-date DATE --target TARGET --signature-file PATH --url URL",
  ].join("\n");
}

function options(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!flag?.startsWith("--") || value === undefined) throw new Error(usage());
    const key = flag.slice(2);
    if (key in parsed) throw new Error(`duplicate option ${flag}`);
    parsed[key] = value;
  }
  return parsed;
}

async function loadCatalog() {
  return JSON.parse(await readFile(CATALOG_URL, "utf8"));
}

async function main(argv) {
  const [command, ...rest] = argv;
  const flags = options(rest);
  const catalog = await loadCatalog();

  if (command === "validate") {
    const allowed = new Set(["platform", "version", "build"]);
    for (const key of Object.keys(flags)) {
      if (!allowed.has(key)) throw new Error(`unknown option --${key}\n${usage()}`);
    }
    validateCatalog(catalog, flags);
    process.stdout.write(`release notes valid (${catalog.releases.length} releases)\n`);
    return;
  }

  if (command === "render") {
    const allowed = new Set(["release", "format"]);
    for (const key of Object.keys(flags)) {
      if (!allowed.has(key)) throw new Error(`unknown option --${key}\n${usage()}`);
    }
    if (!flags.release || !flags.format) throw new Error(usage());
    validateCatalog(catalog);
    const release = catalog.releases.find((entry) => entry.id === flags.release);
    if (!release) throw new Error(`unknown release ${flags.release}`);
    process.stdout.write(renderRelease(release, flags.format));
    return;
  }

  if (command === "updater-manifest") {
    const allowed = new Set([
      "version",
      "notes-file",
      "pub-date",
      "target",
      "signature-file",
      "url",
    ]);
    for (const key of Object.keys(flags)) {
      if (!allowed.has(key)) throw new Error(`unknown option --${key}\n${usage()}`);
    }
    for (const key of allowed) {
      if (!flags[key]) throw new Error(`--${key} is required\n${usage()}`);
    }
    const [notes, signature] = await Promise.all([
      readFile(flags["notes-file"], "utf8"),
      readFile(flags["signature-file"], "utf8"),
    ]);
    process.stdout.write(buildUpdaterManifest({
      version: flags.version,
      notes,
      pubDate: flags["pub-date"],
      target: flags.target,
      signature: signature.trim(),
      url: flags.url,
    }));
    return;
  }

  throw new Error(usage());
}

main(process.argv.slice(2)).catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
