import { mkdirSync, appendFileSync } from "node:fs";
import { dirname } from "node:path";
import type { Page, TestInfo } from "@playwright/test";

type Timing = {
  name: string;
  ms: number;
  budget_ms: number;
  ok: boolean;
};

export async function recordTiming(
  testInfo: TestInfo,
  name: string,
  ms: number,
  budgetMs: number,
): Promise<void> {
  const timing: Timing = {
    name,
    ms: Math.round(ms),
    budget_ms: budgetMs,
    ok: ms <= budgetMs,
  };
  await testInfo.attach(`${name}.json`, {
    body: JSON.stringify(timing, null, 2),
    contentType: "application/json",
  });

  const out = process.env.TESELA_PERF_TIMINGS;
  if (out) {
    mkdirSync(dirname(out), { recursive: true });
    appendFileSync(out, `${JSON.stringify(timing)}\n`);
  }
}

export async function timed<T>(
  testInfo: TestInfo,
  name: string,
  budgetMs: number,
  fn: () => Promise<T>,
): Promise<T> {
  const start = performance.now();
  const value = await fn();
  await recordTiming(testInfo, name, performance.now() - start, budgetMs);
  return value;
}

export async function expectNoConsoleErrors(page: Page): Promise<void> {
  const messages: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") messages.push(msg.text());
  });
  page.on("pageerror", (err) => messages.push(err.message));
  await page.evaluate(() => undefined);
  if (messages.length > 0) {
    throw new Error(`Console errors:\n${messages.join("\n")}`);
  }
}
