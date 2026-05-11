import { expect, test } from "@playwright/test";
import { expectNoConsoleErrors, timed } from "./perf-utils";

test.describe("Tesela perf smoke suite", () => {
  test("Dailies first paint reaches five visible day sections inside budget", async ({ page }, testInfo) => {
    await timed(testInfo, "dailies-first-paint", 1_500, async () => {
      await page.goto("/p/dailies");
      await page.waitForSelector(".journal > .day:nth-child(5)", { timeout: 1_500 });
    });
    await expectNoConsoleErrors(page);
  });

  test("rail navigation to Tasks kanban shows the first card inside budget", async ({ page }, testInfo) => {
    await page.addInitScript(() => {
      localStorage.setItem("tesela:tag-view:tasks", "kanban");
    });
    await page.goto("/p/dailies");
    await expect(page.locator('a[href="/p/tasks"]')).toBeVisible();

    await timed(testInfo, "rail-to-tasks-kanban", 800, async () => {
      await page.locator('a[href="/p/tasks"]').click();
      await expect(page.locator(".kanban-card").first()).toBeVisible({ timeout: 800 });
    });
    await expectNoConsoleErrors(page);
  });

  test("command palette opens and renders its first result inside budget", async ({ page }, testInfo) => {
    await page.goto("/p/dailies");
    await page.locator(".cm-content").first().focus();

    await timed(testInfo, "command-palette-open", 300, async () => {
      await page.keyboard.press("Control+K");
      await expect(page.locator('input[placeholder^="Search commands"]')).toBeVisible({ timeout: 300 });
      await expect(page.getByText(/^(Recent|Actions|Create)$/).first()).toBeVisible({ timeout: 300 });
    });
    await expectNoConsoleErrors(page);
  });

  test("Settings Mosaic create-plus-Logseq-import reaches plan preview inside budget", async ({ page }, testInfo) => {
    const source = process.env.TESELA_PERF_LOGSEQ_SOURCE;
    const target = process.env.TESELA_PERF_NEW_MOSAIC;
    expect(source, "TESELA_PERF_LOGSEQ_SOURCE").toBeTruthy();
    expect(target, "TESELA_PERF_NEW_MOSAIC").toBeTruthy();

    await page.goto("/settings/mosaic");
    await page.getByRole("button", { name: "From another tool" }).click();
    await page.getByRole("button", { name: "Custom folder" }).click();
    await page.getByPlaceholder(/my-new-mosaic|new mosaic|path\/to\/new\/mosaic/i).fill(target!);
    await page.getByRole("button", { name: "Logseq" }).click();
    await page.getByPlaceholder("Source folder to import").fill(source!);

    await timed(testInfo, "logseq-import-plan", 5_000, async () => {
      await page.getByRole("button", { name: "Create + import" }).click();
      await expect(page.getByText("Review the Logseq import below before applying.")).toBeVisible({
        timeout: 5_000,
      });
      await expect(page.getByText(/item(s)? from/)).toBeVisible();
    });
    await expectNoConsoleErrors(page);
  });
});
