import { test, expect } from '@playwright/test';
test('dark picker screenshot and real-browser keyboard selection', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByLabel('Search actions')).toBeFocused();
  await expect(page).toHaveScreenshot('picker.png', { animations: 'disabled', fullPage: true, maxDiffPixelRatio: 0.002 });
  await page.getByLabel('Search actions').fill('stscn');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('treeitem', { name: 'Set scene', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('status')).toHaveText('Added: Set scene (OBS)');
  await page.getByRole('button', { name: '＋ Add sub-action' }).click();
  await page.getByRole('button', { name: /Recent/ }).click();
  await expect(page.getByRole('treeitem', { name: 'Set scene', exact: true })).toBeVisible();
  await page.reload();
  await page.getByRole('button', { name: /Recent/ }).click();
  await page.getByRole('treeitem', { name: 'OBS', exact: true }).click();
  await expect(page.getByRole('treeitem', { name: 'Set scene', exact: true })).toBeVisible();
});

test('narrow layout stays within the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Add sub-action', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.getByLabel('Search actions').fill('obs');
  await page.getByRole('treeitem', { name: 'Set scene', exact: true }).click();
  await page.getByRole('button', { name: '☆ Add favourite' }).click();
  await page.reload();
  await page.getByRole('button', { name: /Favourites/ }).click();
  await page.getByRole('treeitem', { name: 'OBS', exact: true }).click();
  await expect(page.getByRole('treeitem', { name: 'Set scene', exact: true })).toBeVisible();
});
