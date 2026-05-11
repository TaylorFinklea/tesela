import { spawn } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";

const here = path.dirname(new URL(import.meta.url).pathname);
const webRoot = path.resolve(here, "../..");
const repoRoot = path.resolve(webRoot, "..");
const tempRoot = path.join(tmpdir(), `tesela-perf-${process.pid}-${Date.now()}`);
const mosaic = path.join(tempRoot, "medium-mosaic");
const importTarget = path.join(tempRoot, "import-target");
const logseqSource = path.join(tempRoot, "small-logseq");
const timingsPath = path.join(webRoot, "test-results", "perf-timings.jsonl");

const children = new Set();

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
    child.on("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`${cmd} ${args.join(" ")} exited with ${code ?? signal}`));
    });
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
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`Timed out waiting for ${url}: ${lastError ?? "no response"}`);
}

function writeSmallLogseqGraph(root) {
  mkdirSync(path.join(root, "pages"), { recursive: true });
  mkdirSync(path.join(root, "journals"), { recursive: true });
  writeFileSync(
    path.join(root, "pages", "Perf Import.md"),
    "- Imported page from Logseq\n- TODO keep import planning fast\n  status:: todo\n",
  );
  writeFileSync(
    path.join(root, "journals", "2026_05_11.md"),
    "- Daily import line\n- DONE already finished\n",
  );
}

function cleanup() {
  for (const child of children) {
    child.kill("SIGTERM");
  }
  rmSync(tempRoot, { recursive: true, force: true });
}

process.on("SIGINT", () => {
  cleanup();
  process.exit(130);
});
process.on("SIGTERM", () => {
  cleanup();
  process.exit(143);
});

try {
  rmSync(timingsPath, { force: true });
  mkdirSync(tempRoot, { recursive: true });
  writeSmallLogseqGraph(logseqSource);

  await runChecked("cargo", [
    "run",
    "-p",
    "tesela-fixtures-cli",
    "--",
    "--preset",
    "medium",
    "--out",
    mosaic,
  ]);

  run("cargo", ["run", "-p", "tesela-server"], {
    env: {
      TESELA_DEFAULT_MOSAIC: mosaic,
      TESELA_SERVER_BIND: "127.0.0.1:7474",
      RUST_LOG: "error",
    },
  });
  await waitFor("http://127.0.0.1:7474/health", 30_000);

  run("pnpm", ["dev", "--host", "127.0.0.1", "--port", "4174"], {
    cwd: webRoot,
    env: { TESELA_PERF: "1" },
  });
  await waitFor("http://127.0.0.1:4174/p/dailies", 30_000);

  await runChecked("pnpm", ["exec", "playwright", "test", "--config", "playwright.perf.config.ts"], {
    cwd: webRoot,
    env: {
      TESELA_PERF_BASE_URL: "http://127.0.0.1:4174",
      TESELA_PERF_LOGSEQ_SOURCE: logseqSource,
      TESELA_PERF_NEW_MOSAIC: importTarget,
      TESELA_PERF_TIMINGS: timingsPath,
    },
  });
} finally {
  cleanup();
}
