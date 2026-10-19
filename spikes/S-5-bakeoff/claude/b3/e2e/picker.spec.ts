import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  // The picker focuses its search box on mount; wait for that before typing.
  await expect(page.getByRole("searchbox", { name: "Search sub-actions" })).toBeFocused();
});

test("default view", async ({ page }) => {
  const picker = page.getByRole("region", { name: "Add sub-action" });
  await expect(picker).toHaveScreenshot("picker-default.png");
});

test("search, keyboard navigation and selection", async ({ page }) => {
  const picker = page.getByRole("region", { name: "Add sub-action" });
  await page.keyboard.type("scene");
  await expect(page.getByRole("status")).toHaveText(/sub-actions match/);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("treeitem", { selected: true })).toBeFocused();
  await expect(picker).toHaveScreenshot("picker-search.png");

  await page.keyboard.press("Alt+f");
  await page.keyboard.press("Enter");
  await expect(page.getByText(/^Added: /)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("treeitem", { name: "Favourites" })).toHaveAttribute("aria-expanded", "true");
  await expect(page.getByRole("treeitem", { name: "Recently used" })).toHaveAttribute("aria-expanded", "true");
  await expect(picker).toHaveScreenshot("picker-favourites.png");
});
