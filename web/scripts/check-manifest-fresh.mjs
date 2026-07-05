#!/usr/bin/env node
/**
 * Check that the checked-in command-manifest.json is fresh (matches the
 * live registry). This script regenerates the manifest and compares it to
 * the checked-in version; if they differ, the check fails.
 *
 * Run via CI or manually: `node scripts/check-manifest-fresh.mjs`
 */
import { createServer } from "vite";
import { writeFileSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const webRoot = fileURLToPath(new URL("..", import.meta.url));
const manifestPath = path.join(webRoot, "src/lib/command-manifest.json");

const server = await createServer({
  root: webRoot,
  server: { middlewareMode: true, hmr: false },
  appType: "custom",
  logLevel: "warn",
});

try {
  const { commandRegistry } = await server.ssrLoadModule(
    "/src/lib/command-registry.svelte.ts",
  );

  const { registerBuiltinCommands } = await server.ssrLoadModule(
    "/src/lib/commands/index.ts",
  );
  registerBuiltinCommands();

  const { readdirSync } = await import("node:fs");
  const editorCommandsDir = path.join(webRoot, "src/lib/editor/commands");
  const editorFiles = readdirSync(editorCommandsDir)
    .filter((f) => f.endsWith(".ts"))
    .sort();
  for (const file of editorFiles) {
    await server.ssrLoadModule(`/src/lib/editor/commands/${file}`);
  }

  const liveManifest = commandRegistry.manifest();
  const liveJson = JSON.stringify(liveManifest, null, 2) + "\n";
  const checkedInJson = readFileSync(manifestPath, "utf8");

  if (liveJson !== checkedInJson) {
    console.error(`❌ command-manifest.json is stale`);
    console.error(`Run 'npm run generate:commands' to refresh it.`);
    process.exit(1);
  }

  console.log(`✓ command-manifest.json is fresh`);
} finally {
  await server.close();
}
