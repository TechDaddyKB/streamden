import { expect, test } from "@playwright/test";

test("picker screenshots cover the default tree, search, and a recent selection", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Add sub-action" }).click();
  const dialog = page.getByTestId("subaction-picker");
  await expect(dialog.getByRole("searchbox", { name: "Search sub-actions" })).toBeFocused();
  await expect(dialog.getByRole("treeitem", { name: "Logic", exact: true })).toBeVisible();
  await expect(dialog).toHaveScreenshot("picker-default.png");

  await dialog.getByRole("searchbox", { name: "Search sub-actions" }).fill("send announcement");
  const match = dialog.getByRole("treeitem", { name: /Send Announcement, Twitch \/ Chat$/ });
  await expect(match).toHaveAttribute("aria-selected", "true");
  await expect(dialog).toHaveScreenshot("picker-search.png");

  await page.keyboard.press("Enter");
  await expect(page.getByRole("list")).toContainText("Send Announcement");
  await page.getByRole("button", { name: "Add sub-action" }).click();
  await expect(dialog.getByRole("treeitem", { name: /Recently used/ })).toBeVisible();
  await expect(dialog).toHaveScreenshot("picker-recent.png");
});
