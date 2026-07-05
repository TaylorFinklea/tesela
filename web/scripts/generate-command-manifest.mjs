/**
 * Generates `src/lib/command-manifest.json` — the checked-in, closure-free
 * snapshot of EVERY built-in command (tesela-cmdd.2). This is the ONE
 * extraction point that produces the file the Rust `GET /commands` route
 * embeds (`crates/tesela-server/src/routes/commands.rs`): both sides derive
 * from the same live `commandRegistry`, so there is never a hand-copied
 * second list to drift.
 *
 * Loads the REAL command set — `registerBuiltinCommands()` (commands/index.ts)
 * plus every `editor/commands/*.ts` module (side-effect registration) — via
 * Vite's `ssrLoadModule`, which resolves the `$lib`/`$app` aliases and
 * compiles the `.svelte.ts` rune files the way the real app does (plain
 * `node --test` can't: it neither resolves those aliases nor understands TS
 * parameter-property syntax used deep in the import graph, e.g.
 * `api-client.ts`'s `ApiError`).
 *
 * Run via `npm run generate:commands` after adding/editing a command.
 */
import { createServer } from "vite";
import { writeFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const webRoot = fileURLToPath(new URL("..", import.meta.url));
const outFile = path.join(webRoot, "src/lib/command-manifest.json");

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

  const editorCommandsDir = path.join(webRoot, "src/lib/editor/commands");
  const editorFiles = readdirSync(editorCommandsDir)
    .filter((f) => f.endsWith(".ts"))
    .sort();
  for (const file of editorFiles) {
    await server.ssrLoadModule(`/src/lib/editor/commands/${file}`);
  }

  const manifest = commandRegistry.manifest();
  if (manifest.length === 0) {
    throw new Error("generate-command-manifest: registry is empty after registration");
  }

  writeFileSync(outFile, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`Wrote ${manifest.length} commands to ${path.relative(webRoot, outFile)}`);
} finally {
  await server.close();
}
