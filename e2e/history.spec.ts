import { test, expect, cell, markCorrect, s, DAY_ONE_L1 } from "./fixtures.ts";

test("undo and redo walk the mark back and forward", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  const target = cell(page, 0, 0);
  await target.click();
  await expect(target).toHaveClass(/incorrect/);

  await page.getByRole("button", { name: s.puzzle.undo }).click();
  await expect(target).not.toHaveClass(/incorrect/);

  await page.getByRole("button", { name: s.puzzle.redo }).click();
  await expect(target).toHaveClass(/incorrect/);
});

test("undo is unavailable on a fresh board", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await expect(page.getByRole("button", { name: s.puzzle.undo })).toBeDisabled();
  await expect(page.getByRole("button", { name: s.puzzle.redo })).toBeDisabled();
});

test("the history strip jumps back to the start", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await markCorrect(page, 0, 0);
  const target = cell(page, 0, 0);
  await expect(target).toHaveClass(/correct/);

  await page.getByRole("button", { name: s.puzzle.start }).click();

  await expect(target).not.toHaveClass(/correct/);
  await expect(target).not.toHaveClass(/incorrect/);
});

test("marks survive a reload", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await markCorrect(page, 0, 0);
  await cell(page, 1, 1).click();

  await page.reload();

  await expect(cell(page, 0, 0)).toHaveClass(/correct/);
  await expect(cell(page, 1, 1)).toHaveClass(/incorrect/);
});
