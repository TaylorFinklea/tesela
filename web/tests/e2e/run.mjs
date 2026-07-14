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
const rawPlaywrightArgs = process.argv.slice(2);
const playwrightArgs = rawPlaywrightArgs[0] === "--" ? rawPlaywrightArgs.slice(1) : rawPlaywrightArgs;

const BIDS = {
  sameBeforeTarget: "10000000-0000-4000-8000-000000000001",
  sameBeforeRoot: "10000000-0000-4000-8000-000000000002",
  sameInsideTarget: "10000000-0000-4000-8000-000000000003",
  sameInsideTargetChild: "10000000-0000-4000-8000-000000000004",
  sameInsideRoot: "10000000-0000-4000-8000-000000000005",
  sameAfterRoot: "10000000-0000-4000-8000-000000000006",
  sameAfterRootChild: "10000000-0000-4000-8000-000000000007",
  sameAfterTarget: "10000000-0000-4000-8000-000000000008",
  sameAfterTargetChild: "10000000-0000-4000-8000-000000000009",
  crossRoot: "20000000-0000-4000-8000-000000000001",
  crossChild: "20000000-0000-4000-8000-000000000002",
  crossGrandchild: "20000000-0000-4000-8000-000000000003",
  existingAppendRoot: "20000000-0000-4000-8000-000000000004",
  existingAppendChild: "20000000-0000-4000-8000-000000000005",
  absentAppendRoot: "20000000-0000-4000-8000-000000000006",
  invalidRoot: "20000000-0000-4000-8000-000000000007",
  invalidChild: "20000000-0000-4000-8000-000000000008",
  invalidTarget: "20000000-0000-4000-8000-000000000009",
  keyboardCancelRoot: "20000000-0000-4000-8000-000000000010",
  keyboardBeforeRoot: "20000000-0000-4000-8000-000000000011",
  keyboardInsideRoot: "20000000-0000-4000-8000-000000000012",
  keyboardAfterRoot: "20000000-0000-4000-8000-000000000013",
  retryRoot: "20000000-0000-4000-8000-000000000014",
  altParent: "20000000-0000-4000-8000-000000000015",
  altMover: "20000000-0000-4000-8000-000000000016",
  altSibling: "20000000-0000-4000-8000-000000000017",
  racePointerRoot: "20000000-0000-4000-8000-000000000018",
  raceAltRoot: "20000000-0000-4000-8000-000000000019",
  raceAltSibling: "20000000-0000-4000-8000-000000000020",
  crossBeforeRoot: "20000000-0000-4000-8000-000000000021",
  crossBeforeChild: "20000000-0000-4000-8000-000000000022",
  crossAfterRoot: "20000000-0000-4000-8000-000000000023",
  crossAfterChild: "20000000-0000-4000-8000-000000000024",
  untrustedFocusRoot: "20000000-0000-4000-8000-000000000025",
  ambiguousRoot: "20000000-0000-4000-8000-000000000026",
  propertyRaceRoot: "20000000-0000-4000-8000-000000000027",
  propertyFailureRoot: "20000000-0000-4000-8000-000000000028",
  crossTarget: "30000000-0000-4000-8000-000000000001",
  crossTargetChild: "30000000-0000-4000-8000-000000000002",
  existingEnd: "30000000-0000-4000-8000-000000000003",
  keyboardBeforeTarget: "30000000-0000-4000-8000-000000000004",
  keyboardInsideTarget: "30000000-0000-4000-8000-000000000005",
  keyboardInsideTargetChild: "30000000-0000-4000-8000-000000000006",
  keyboardAfterTarget: "30000000-0000-4000-8000-000000000007",
  keyboardAfterTargetChild: "30000000-0000-4000-8000-000000000008",
  retryTarget: "30000000-0000-4000-8000-000000000009",
  racePointerTarget: "30000000-0000-4000-8000-000000000010",
  crossBeforeTarget: "30000000-0000-4000-8000-000000000011",
  crossBeforeTargetChild: "30000000-0000-4000-8000-000000000012",
  crossAfterTarget: "30000000-0000-4000-8000-000000000013",
  crossAfterTargetChild: "30000000-0000-4000-8000-000000000014",
  untrustedFocusTarget: "30000000-0000-4000-8000-000000000015",
  ambiguousTarget: "30000000-0000-4000-8000-000000000016",
  propertyRaceTarget: "30000000-0000-4000-8000-000000000017",
  propertyFailureTarget: "30000000-0000-4000-8000-000000000018",
};

const children = new Set();
const PROCESS_TREE_GRACE_MS = 5_000;
const PROCESS_TREE_KILL_MS = 5_000;
let cleanupPromise;
let requestedExitCode = null;

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
  if (requestedExitCode !== null) {
    throw new Error(`Refusing to start ${cmd} during shutdown`);
  }
  const child = spawn(cmd, args, {
    cwd: options.cwd ?? repoRoot,
    env: { ...process.env, ...(options.env ?? {}) },
    stdio: options.stdio ?? "inherit",
    detached: process.platform !== "win32",
  });
  children.add(child);
  return child;
}

function runChecked(cmd, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = run(cmd, args, options);
    child.once("error", reject);
    child.once("exit", (code, signal) =>
      code === 0 ? resolve() : reject(new Error(`${cmd} ${args.join(" ")} exited ${code ?? signal}`)),
    );
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function processTreeAlive(child) {
  if (!child.pid) return false;
  if (process.platform === "win32") {
    return child.exitCode === null && child.signalCode === null;
  }
  try {
    process.kill(-child.pid, 0);
    return true;
  } catch (err) {
    if (err?.code === "ESRCH") return false;
    if (err?.code === "EPERM") return true;
    throw err;
  }
}

async function signalProcessTree(child, signal) {
  if (!child.pid || !processTreeAlive(child)) return;
  if (process.platform === "win32") {
    await new Promise((resolve) => {
      const killer = spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
        stdio: "ignore",
        windowsHide: true,
      });
      killer.once("error", resolve);
      killer.once("exit", resolve);
    });
    return;
  }
  try {
    process.kill(-child.pid, signal);
  } catch (err) {
    if (err?.code !== "ESRCH") throw err;
  }
}

async function waitForProcessTrees(childrenToWaitFor, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let live = childrenToWaitFor.filter(processTreeAlive);
  while (live.length > 0 && Date.now() < deadline) {
    await delay(50);
    live = live.filter(processTreeAlive);
  }
  return live;
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

function previousIsoDate(value) {
  const [year, month, day] = value.split("-").map(Number);
  const date = new Date(Date.UTC(year, month - 1, day));
  date.setUTCDate(date.getUTCDate() - 1);
  return date.toISOString().slice(0, 10);
}

async function createDaily(apiBase, title, body) {
  const content = `---\ntitle: "${title}"\ntags: [daily]\ncreated: ${title}T00:00:00Z\n---\n${body}`;
  const response = await fetch(`${apiBase}/notes`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ title, content, tags: ["daily"] }),
  });
  if (!response.ok) {
    throw new Error(`seed daily ${title} failed: ${response.status} ${await response.text()}`);
  }
}

function cleanup() {
  cleanupPromise ??= (async () => {
    const tracked = [...children].reverse();
    const live = tracked.filter(processTreeAlive);
    await Promise.all(live.map((child) => signalProcessTree(child, "SIGTERM")));
    let remaining = await waitForProcessTrees(live, PROCESS_TREE_GRACE_MS);
    if (remaining.length > 0) {
      await Promise.all(remaining.map((child) => signalProcessTree(child, "SIGKILL")));
      remaining = await waitForProcessTrees(remaining, PROCESS_TREE_KILL_MS);
    }
    if (remaining.length > 0) {
      const pids = remaining.map((child) => child.pid).join(", ");
      throw new Error(`process trees did not exit before cleanup: ${pids}`);
    }
    children.clear();
    rmSync(tempRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  })();
  return cleanupPromise;
}

function handleSignal(exitCode) {
  if (requestedExitCode !== null) return;
  requestedExitCode = exitCode;
  void cleanup().then(
    () => process.exit(exitCode),
    (err) => {
      console.error(`[e2e] cleanup failed: ${err?.message ?? err}`);
      process.exit(exitCode);
    },
  );
}

process.once("SIGINT", () => handleSignal(130));
process.once("SIGTERM", () => handleSignal(143));

let exitCode = 0;
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
      TESELA_DISABLE_MDNS: "1",
      TESELA_DISABLE_PEER_SYNC: "1",
      TESELA_GROUP_KEY_FILE_STORE: "1",
      TESELA_BACKUP_ON_START: "0",
      TESELA_BACKUP_INTERVAL_SECS: "0",
      RUST_LOG: "error",
    },
  });
  const apiBase = `http://127.0.0.1:${apiPort}`;
  await waitFor(`${apiBase}/health`, 60_000);

  const dailyResponse = await fetch(`${apiBase}/notes?tag=daily&limit=100`);
  if (!dailyResponse.ok) throw new Error(`list fixture dailies failed: ${dailyResponse.status}`);
  const existingDailies = await dailyResponse.json();
  const oldestDaily = existingDailies
    .map((note) => note.title)
    .filter((title) => /^\d{4}-\d{2}-\d{2}$/.test(title))
    .sort()[0];
  if (!oldestDaily) throw new Error("tiny fixture did not contain an ISO daily");
  const SOURCE_DAILY = previousIsoDate(oldestDaily);
  const DEST_DAILY = previousIsoDate(SOURCE_DAILY);
  const ABSENT_DAILY = previousIsoDate(DEST_DAILY);

  await createDaily(apiBase, SOURCE_DAILY, [
    `- SAME_BEFORE_TARGET <!-- bid:${BIDS.sameBeforeTarget} -->`,
    `- SAME_BEFORE_ROOT <!-- bid:${BIDS.sameBeforeRoot} -->`,
    `- SAME_INSIDE_TARGET <!-- bid:${BIDS.sameInsideTarget} -->`,
    `  - SAME_INSIDE_TARGET_CHILD <!-- bid:${BIDS.sameInsideTargetChild} -->`,
    `- SAME_INSIDE_ROOT <!-- bid:${BIDS.sameInsideRoot} -->`,
    `- SAME_AFTER_ROOT <!-- bid:${BIDS.sameAfterRoot} -->`,
    `  - SAME_AFTER_ROOT_CHILD <!-- bid:${BIDS.sameAfterRootChild} -->`,
    `- SAME_AFTER_TARGET <!-- bid:${BIDS.sameAfterTarget} -->`,
    `  - SAME_AFTER_TARGET_CHILD <!-- bid:${BIDS.sameAfterTargetChild} -->`,
    `- CROSS_ROOT <!-- bid:${BIDS.crossRoot} -->`,
    `  - CROSS_CHILD <!-- bid:${BIDS.crossChild} -->`,
    `    - CROSS_GRANDCHILD <!-- bid:${BIDS.crossGrandchild} -->`,
    `- EXISTING_APPEND_ROOT <!-- bid:${BIDS.existingAppendRoot} -->`,
    `  - EXISTING_APPEND_CHILD <!-- bid:${BIDS.existingAppendChild} -->`,
    `- ABSENT_APPEND_ROOT <!-- bid:${BIDS.absentAppendRoot} -->`,
    `- INVALID_ROOT <!-- bid:${BIDS.invalidRoot} -->`,
    `  - INVALID_CHILD <!-- bid:${BIDS.invalidChild} -->`,
    `- INVALID_TARGET <!-- bid:${BIDS.invalidTarget} -->`,
    `- KEYBOARD_CANCEL_ROOT <!-- bid:${BIDS.keyboardCancelRoot} -->`,
    `- KEYBOARD_BEFORE_ROOT <!-- bid:${BIDS.keyboardBeforeRoot} -->`,
    `- KEYBOARD_INSIDE_ROOT <!-- bid:${BIDS.keyboardInsideRoot} -->`,
    `- KEYBOARD_AFTER_ROOT <!-- bid:${BIDS.keyboardAfterRoot} -->`,
    `- RETRY_ROOT <!-- bid:${BIDS.retryRoot} -->`,
    `- ALT_PARENT <!-- bid:${BIDS.altParent} -->`,
    `- ALT_MOVER <!-- bid:${BIDS.altMover} -->`,
    `- ALT_SIBLING <!-- bid:${BIDS.altSibling} -->`,
    `- RACE_POINTER_ROOT <!-- bid:${BIDS.racePointerRoot} -->`,
    `- RACE_ALT_ROOT <!-- bid:${BIDS.raceAltRoot} -->`,
    `- RACE_ALT_SIBLING <!-- bid:${BIDS.raceAltSibling} -->`,
    `- CROSS_BEFORE_ROOT <!-- bid:${BIDS.crossBeforeRoot} -->`,
    `  - CROSS_BEFORE_CHILD <!-- bid:${BIDS.crossBeforeChild} -->`,
    `- CROSS_AFTER_ROOT <!-- bid:${BIDS.crossAfterRoot} -->`,
    `  - CROSS_AFTER_CHILD <!-- bid:${BIDS.crossAfterChild} -->`,
    `- UNTRUSTED_FOCUS_ROOT <!-- bid:${BIDS.untrustedFocusRoot} -->`,
    `- AMBIGUOUS_ROOT <!-- bid:${BIDS.ambiguousRoot} -->`,
    `- PROPERTY_RACE_ROOT <!-- bid:${BIDS.propertyRaceRoot} -->`,
    `- PROPERTY_FAILURE_ROOT <!-- bid:${BIDS.propertyFailureRoot} -->`,
    `  status:: todo`,
    "",
  ].join("\n"));

  await createDaily(apiBase, DEST_DAILY, [
    `- CROSS_BEFORE_TARGET <!-- bid:${BIDS.crossBeforeTarget} -->`,
    `  - CROSS_BEFORE_TARGET_CHILD <!-- bid:${BIDS.crossBeforeTargetChild} -->`,
    `- CROSS_TARGET <!-- bid:${BIDS.crossTarget} -->`,
    `  - CROSS_TARGET_CHILD <!-- bid:${BIDS.crossTargetChild} -->`,
    `- CROSS_AFTER_TARGET <!-- bid:${BIDS.crossAfterTarget} -->`,
    `  - CROSS_AFTER_TARGET_CHILD <!-- bid:${BIDS.crossAfterTargetChild} -->`,
    `- EXISTING_END <!-- bid:${BIDS.existingEnd} -->`,
    `- KEYBOARD_BEFORE_TARGET <!-- bid:${BIDS.keyboardBeforeTarget} -->`,
    `- KEYBOARD_INSIDE_TARGET <!-- bid:${BIDS.keyboardInsideTarget} -->`,
    `  - KEYBOARD_INSIDE_TARGET_CHILD <!-- bid:${BIDS.keyboardInsideTargetChild} -->`,
    `- KEYBOARD_AFTER_TARGET <!-- bid:${BIDS.keyboardAfterTarget} -->`,
    `  - KEYBOARD_AFTER_TARGET_CHILD <!-- bid:${BIDS.keyboardAfterTargetChild} -->`,
    `- RETRY_TARGET <!-- bid:${BIDS.retryTarget} -->`,
    `- RACE_POINTER_TARGET <!-- bid:${BIDS.racePointerTarget} -->`,
    `- UNTRUSTED_FOCUS_TARGET <!-- bid:${BIDS.untrustedFocusTarget} -->`,
    `- AMBIGUOUS_TARGET <!-- bid:${BIDS.ambiguousTarget} -->`,
    `- PROPERTY_RACE_TARGET <!-- bid:${BIDS.propertyRaceTarget} -->`,
    `- PROPERTY_FAILURE_TARGET <!-- bid:${BIDS.propertyFailureTarget} -->`,
    "",
  ].join("\n"));

  for (const [bid, value] of [
    [BIDS.crossRoot, "doing"],
    [BIDS.propertyRaceRoot, "todo"],
    [BIDS.propertyFailureRoot, "todo"],
  ]) {
    const propertyResponse = await fetch(`${apiBase}/blocks/set-property`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        block_id: `${SOURCE_DAILY}:${bid}`,
        key: "status",
        value,
      }),
    });
    if (!propertyResponse.ok) {
      throw new Error(`seed relocation property failed: ${propertyResponse.status} ${await propertyResponse.text()}`);
    }
  }

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
    env: { TESELA_API_TARGET: apiBase },
  });
  const webBase = `http://127.0.0.1:${webPort}`;
  await waitFor(`${webBase}/g`, 60_000);

  if (process.env.TESELA_E2E_QA_HOLD === "1") {
    console.log(`[e2e] QA hold: ${webBase}/g`);
    console.log(`[e2e] API: ${apiBase}`);
    console.log(`[e2e] relocation dates: source=${SOURCE_DAILY} destination=${DEST_DAILY} absent=${ABSENT_DAILY}`);
    console.log("[e2e] press Ctrl-C to clean up");
    await new Promise(() => {});
  }

  await runChecked("pnpm", ["exec", "playwright", "test", "--config", "playwright.e2e.config.ts", ...playwrightArgs], {
    cwd: webRoot,
    env: {
      TESELA_E2E_BASE_URL: webBase,
      TESELA_E2E_DAILY_SLUG: DAILY,
      TESELA_E2E_SOURCE_DAILY: SOURCE_DAILY,
      TESELA_E2E_DEST_DAILY: DEST_DAILY,
      TESELA_E2E_ABSENT_DAILY: ABSENT_DAILY,
    },
  });
} catch (err) {
  exitCode = requestedExitCode ?? 1;
  if (requestedExitCode === null) console.error(`[e2e] failed: ${err?.message ?? err}`);
}

try {
  await cleanup();
} catch (err) {
  console.error(`[e2e] cleanup failed: ${err?.message ?? err}`);
  exitCode = requestedExitCode ?? 1;
}
process.exit(exitCode);
