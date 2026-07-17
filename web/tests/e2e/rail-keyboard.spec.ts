import { expect, test } from "@playwright/test";

const SLUG = process.env.TESELA_E2E_DAILY_SLUG ?? "e2e-delete-refresh";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ slug }) => {
    localStorage.setItem("tesela:favorites", JSON.stringify([slug]));
    localStorage.setItem("tesela:workspace:pinned", JSON.stringify([slug]));
  }, { slug: SLUG });
});

test("leader enters the rail, traversal wraps, Escape returns, and Enter invokes", async ({ page }) => {
  await page.goto("/g");
  const releaseDone = page.getByRole("button", { name: "Done", exact: true });
  await expect(releaseDone).toBeVisible();
  await releaseDone.click();
  await expect(releaseDone).toHaveCount(0);

  const origin = page.getByRole("button", { name: /Search or run a command/ });
  await origin.focus();

  await page.keyboard.press("Space");
  await page.keyboard.press("r");
  await page.keyboard.press("f");

  const actions = page.locator(".gr-rail [data-rail-action]");
  await expect(actions.first()).toBeFocused();
  expect(await actions.count()).toBeGreaterThanOrEqual(4);

  const commandIds = await actions.evaluateAll((elements) =>
    elements.map((element) => element.getAttribute("data-command-id")),
  );
  expect(commandIds).not.toContain(null);
  expect(new Set(commandIds)).toEqual(new Set([
    "rail-quick-capture",
    "jump",
    "rail-toggle-favorite",
    "rail-add-widget",
  ]));

  await page.keyboard.press("End");
  await expect(actions.last()).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByText("Widget customization is coming soon")).toBeVisible();
  await page.keyboard.press("j");
  await expect(actions.first()).toBeFocused();
  await page.keyboard.press("k");
  await expect(actions.last()).toBeFocused();
  await page.keyboard.press("Home");
  await expect(actions.first()).toBeFocused();

  await page.keyboard.press("j");
  await expect(actions.nth(1)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator(".gr-pane-head .ttl").filter({ hasText: SLUG }).first()).toBeVisible();

  await page.keyboard.press("j");
  await expect(actions.nth(2)).toBeFocused();
  await page.keyboard.press("j");
  await expect(actions.nth(3)).toBeFocused();
  await page.keyboard.press("j");
  await expect(actions.nth(4)).toBeFocused();
  await page.keyboard.press("Enter");
  const pinnedWidget = page.locator(".gr-w").filter({
    has: page.locator(".gr-w-head .ti", { hasText: "Pinned" }),
  });
  const pinnedStar = pinnedWidget.getByRole("button", {
    name: `Add ${SLUG} to favorites`,
  });
  await expect(pinnedStar).toBeFocused();

  await page.keyboard.press("Escape");
  await expect(origin).toBeFocused();

  await page.keyboard.press("Space");
  await page.keyboard.press("r");
  await page.keyboard.press("f");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog", { name: "vim ex command" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "vim ex command" })).toHaveCount(0);

  await actions.first().focus();
  await page.keyboard.press("Space");
  await expect(page.getByRole("dialog", { name: "vim ex command" })).toBeVisible();
  await page.keyboard.press("Escape");

  await origin.focus();
  await actions.first().focus();
  const settings = page.getByRole("button", { name: "Settings", exact: true });
  await settings.focus();
  await actions.first().focus();
  await page.keyboard.press("Escape");
  await expect(settings).toBeFocused();
});

test("Escape restores a Daily editor after the keydown completes", async ({ page }) => {
  await page.addInitScript(() => {
    const nativeFocus = HTMLElement.prototype.focus;
    let dispatchingEscape = false;

    document.addEventListener("keydown", (event) => {
      if (event.key !== "Escape") return;
      dispatchingEscape = true;
      requestAnimationFrame(() => {
        dispatchingEscape = false;
      });
    }, true);

    HTMLElement.prototype.focus = function focus(options?: FocusOptions) {
      if (dispatchingEscape && this.classList.contains("cm-content")) return;
      nativeFocus.call(this, options);
    };
  });

  await page.goto("/g");
  const releaseDone = page.getByRole("button", { name: "Done", exact: true });
  await expect(releaseDone).toBeVisible();
  await releaseDone.click();

  const dailyEditor = page.locator(".cm-content").first();
  await dailyEditor.click();
  await page.keyboard.press("Escape");
  await expect(dailyEditor).toBeFocused();

  for (let attempt = 0; attempt < 2; attempt += 1) {
    await page.keyboard.press("Space");
    await page.keyboard.press("r");
    await page.keyboard.press("f");

    const actions = page.locator(".gr-rail [data-rail-action]");
    await expect(actions.first()).toBeFocused();
    await page.keyboard.press("End");
    await expect(actions.last()).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dailyEditor).toBeFocused();
  }

  await page.keyboard.press("Space");
  await expect(page.locator(".gr-leader")).toBeVisible();
  await expect(page.getByText("Widget customization is coming soon")).toHaveCount(0);
});

test("deferred Escape restoration does not steal a newer focus target", async ({ page }) => {
  await page.addInitScript(() => {
    const nativeFocus = HTMLElement.prototype.focus;
    let dispatchingEscape = false;

    document.addEventListener("keydown", (event) => {
      const active = document.activeElement;
      if (
        event.key !== "Escape"
        || !(active instanceof HTMLElement)
        || !active.closest("[data-rail-action]")
      ) return;

      dispatchingEscape = true;
      requestAnimationFrame(() => {
        dispatchingEscape = false;
      });
    }, true);

    HTMLElement.prototype.focus = function focus(options?: FocusOptions) {
      if (dispatchingEscape && this.classList.contains("cm-content")) return;
      nativeFocus.call(this, options);
    };
  });

  await page.goto("/g");
  const releaseDone = page.getByRole("button", { name: "Done", exact: true });
  await expect(releaseDone).toBeVisible();
  await releaseDone.click();

  const dailyEditor = page.locator(".cm-content").first();
  await dailyEditor.click();
  await page.keyboard.press("Escape");

  await page.keyboard.press("Space");
  await page.keyboard.press("r");
  await page.keyboard.press("f");
  const actions = page.locator(".gr-rail [data-rail-action]");
  await page.keyboard.press("End");
  await expect(actions.last()).toBeFocused();

  await page.evaluate(() => {
    const nativeRequestAnimationFrame = window.requestAnimationFrame.bind(window);
    let injectNewFocus = true;
    window.requestAnimationFrame = (callback) => nativeRequestAnimationFrame((time) => {
      if (injectNewFocus) {
        injectNewFocus = false;
        document.querySelector<HTMLElement>('button[aria-label="Settings"]')?.focus();
      }
      callback(time);
    });
  });

  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Settings", exact: true })).toBeFocused();
});
