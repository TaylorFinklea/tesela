import { expect, test } from "@playwright/test";

const CURRENT_WEB_RELEASE = "2026-07-15.desktop-0.1.2";
const SEEN_KEY = "tesela:releaseNotes:lastSeen:web";

async function seedCurrentAsSeen(page: import("@playwright/test").Page) {
  await page.addInitScript(
    ({ key, release }) => localStorage.setItem(key, release),
    { key: SEEN_KEY, release: CURRENT_WEB_RELEASE },
  );
}

test("automatically presents the current release once", async ({ page }) => {
  await page.goto("/g");

  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toBeVisible();
  await expect(page.getByText("Tesela Web · Jul 15, 2026")).toBeVisible();
  await expect(page.getByRole("heading", { name: "New", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Fixed", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Important", exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), SEEN_KEY))
    .toBe(CURRENT_WEB_RELEASE);

  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toBeHidden();

  await page.reload();
  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toHaveCount(0);
});

test("Settings opens latest-first history and returns to Settings", async ({ page }) => {
  await seedCurrentAsSeen(page);
  await page.goto("/g");
  await page.getByRole("button", { name: "Settings", exact: true }).click();

  await expect(page.getByRole("dialog", { name: "Settings" })).toBeVisible();
  await page.getByRole("button", { name: "What’s New" }).click();
  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toBeVisible();

  await page.getByRole("button", { name: /View older releases/ }).click();
  await expect(page.getByRole("heading", { name: "Earlier releases" })).toBeVisible();
  await page.getByRole("button", { name: /Find and shape your notes/ }).click();
  await expect(page.getByRole("heading", { name: "Find and shape your notes" })).toBeVisible();

  await page.getByRole("button", { name: "Back" }).click();
  await expect(page.getByRole("heading", { name: "Earlier releases" })).toBeVisible();
  await page.getByRole("button", { name: "Back" }).click();
  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toBeVisible();

  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("dialog", { name: "Settings" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Settings" })).toHaveCount(0);
});

test("the shared command opens What’s New and Escape closes it", async ({ page }) => {
  await seedCurrentAsSeen(page);
  await page.goto("/g");

  await page.getByRole("button", { name: /Search or run a command/ }).click();
  const palette = page.getByPlaceholder("Search or run a command…");
  await expect(palette).toBeVisible();
  await palette.fill("whats-new");
  await page.getByRole("option", { name: /What’s New/ }).click();

  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { name: "Sharper daily work" })).toHaveCount(0);
});
