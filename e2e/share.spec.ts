import { test, expect, cell, markCorrect, s, DAY_ONE, DAY_ONE_L1 } from "./fixtures.ts";

/** The sheet prints the URL with the protocol stripped and no trailing slash. */
async function sheetUrl(page: import("@playwright/test").Page): Promise<string> {
  const shown = await page.locator(".share-sheet-url").innerText();
  return `http://${shown}`;
}

test("the share sheet offers the puzzle's own URL", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await page.getByRole("button", { name: s.puzzle.share, exact: true }).click();

  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await sheetUrl(page)).toMatch(new RegExp(`/${DAY_ONE}/1$`));
});

test("a shared progress URL restores the board on a clean device", async ({ page, browser }) => {
  await page.goto(DAY_ONE_L1);

  await markCorrect(page, 0, 0);
  await cell(page, 1, 1).click();

  await page.getByRole("button", { name: s.puzzle.shareOptions }).click();
  await page.getByRole("menuitem", { name: s.puzzle.shareWithProgress }).click();
  const url = await sheetUrl(page);

  // The hash has to carry the marks on its own, so open it with nothing stored.
  const fresh = await browser.newContext();
  const freshPage = await fresh.newPage();
  await freshPage.addInitScript(() => {
    localStorage.setItem("refpuzzle:onboarded", "1");
    localStorage.setItem("refpuzzle:theme", "light");
  });
  await freshPage.goto(url);

  await expect(freshPage.locator('[data-qi="0"][data-oi="0"]')).toHaveClass(/correct/);
  await expect(freshPage.locator('[data-qi="1"][data-oi="1"]')).toHaveClass(/incorrect/);

  await fresh.close();
});
