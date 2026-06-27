// Self-contained harness for the functional e2e suite (tests/e2e/*.spec.ts).
// Mirrors tests/perf/run.mjs but lighter: a tiny fixture mosaic + a relay-off
// tesela-server + a seeded daily + `pnpm dev` proxied to it, then Playwright.
//
//   pnpm test:e2e
//
import { spawn } from "node:child_process";
import { mkdirSync, rmSync } from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";

const here = path.dirname(new URL(import.meta.url).pathname);
const webRoot = path.resolve(here, "../..");
const repoRoot = path.resolve(webRoot, "..");
const tempRoot = path.join(tmpdir(), `tesela-e2e-${process.pid}-${Date.now()}`);
const mosaic = path.join(tempRoot, "mosaic");

const children = new Set();

function pickFreePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
  });
}

function run(cmd, args, options = {}) {
  const child = spawn(cmd, args, {
    cwd: options.cwd ?? repoRoot,
    env: { ...process.env, ...(options.env ?? {}) },
    stdio: options.stdio ?? "inherit",
  });
  children.add(child);
  child.on("exit", () => children.delete(child));
  return child;
}

function runChecked(cmd, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = run(cmd, args, options);
    child.on("exit", (code, signal) =>
      code === 0 ? resolve() : reject(new Error(`${cmd} ${args.join(" ")} exited ${code ?? signal}`)),
    );
  });
}

async function waitFor(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch (err) {
      lastError = err;
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`Timed out waiting for ${url}: ${lastError ?? "no response"}`);
}

function cleanup() {
  for (const child of children) child.kill("SIGTERM");
  rmSync(tempRoot, { recursive: true, force: true });
}
process.on("SIGINT", () => { cleanup(); process.exit(130); });
process.on("SIGTERM", () => { cleanup(); process.exit(143); });

try {
  mkdirSync(tempRoot, { recursive: true });
  await runChecked("cargo", ["run", "-p", "tesela-fixtures-cli", "--", "--preset", "tiny", "--out", mosaic]);

  const apiPort = await pickFreePort();
  const webPort = await pickFreePort();

  run("cargo", ["run", "-p", "tesela-server"], {
    env: {
      TESELA_DEFAULT_MOSAIC: mosaic,
      TESELA_SERVER_BIND: `127.0.0.1:${apiPort}`,
      TESELA_DISABLE_RELAY: "1",
      RUST_LOG: "error",
    },
  });
  await waitFor(`http://127.0.0.1:${apiPort}/health`, 60_000);

  // Create a dedicated PAGE note (a single BlockOutliner — the bug lives in
  // BlockOutliner.applyExternalReparse, shared by pages + the journal — and a
  // page renders without the journal's view-state setup). Bids are stamped
  // server-side.
  const DAILY = "e2e-delete-refresh";
  const seed = await fetch(`http://127.0.0.1:${apiPort}/notes`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ title: DAILY, content: "- seed block\n", tags: [] }),
  });
  if (!seed.ok) throw new Error(`seed page failed: ${seed.status}`);
  console.log(`[e2e] using page ${DAILY}`);

  run("pnpm", ["dev", "--host", "127.0.0.1", "--port", String(webPort)], {
    cwd: webRoot,
    env: { TESELA_API_TARGET: `http://127.0.0.1:${apiPort}` },
  });
  await waitFor(`http://127.0.0.1:${webPort}/g`, 60_000);

  await runChecked("pnpm", ["exec", "playwright", "test", "--config", "playwright.e2e.config.ts"], {
    cwd: webRoot,
    env: { TESELA_E2E_BASE_URL: `http://127.0.0.1:${webPort}`, TESELA_E2E_DAILY_SLUG: DAILY },
  });
  cleanup();
  process.exit(0);
} catch (err) {
  console.error(`[e2e] failed: ${err?.message ?? err}`);
  cleanup();
  process.exit(1);
}
