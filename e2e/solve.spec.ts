import { test, expect, s, solveDayOneL1, DAY_ONE, DAY_ONE_L1 } from "./fixtures.ts";

/** The stored entry for day-one level 1, ledger included. */
function storedEntry(page: import("@playwright/test").Page) {
  return page.evaluate(() => localStorage.getItem("refpuzzle:puzzle:/2026-04-19/1"));
}

test("solving the board shows the completion banner", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await expect(page.getByText(s.puzzle.solved)).toBeHidden();

  await solveDayOneL1(page);

  await expect(page.getByText(s.puzzle.solved)).toBeVisible();
  // Level 1 of 6, so the banner offers the next level rather than the archive.
  await expect(page.getByRole("button", { name: new RegExp(s.puzzle.nextPuzzle) })).toBeVisible();
});

test("the next-puzzle button moves to level 2", async ({ page }) => {
  await page.goto(DAY_ONE_L1);
  await solveDayOneL1(page);

  await page.getByRole("button", { name: new RegExp(s.puzzle.nextPuzzle) }).click();

  await expect(page.getByRole("tab", { name: s.difficulty[2] })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page).toHaveURL(/\/2026-04-19\/2$/);
});

test("a solved board is still solved after a reload", async ({ page }) => {
  await page.goto(DAY_ONE_L1);
  await solveDayOneL1(page);
  await expect(page.getByText(s.puzzle.solved)).toBeVisible();

  await page.reload();

  await expect(page.getByText(s.puzzle.solved)).toBeVisible();
  // The level tab reports the solve too.
  await expect(page.getByRole("tab", { name: s.difficulty[1] })).toHaveClass(/tab-solved/);
});

test("solving keeps the history the player built", async ({ page }) => {
  await page.goto(DAY_ONE_L1);
  await solveDayOneL1(page);
  await expect(page.getByText(s.puzzle.solved)).toBeVisible();

  // Every mark still in the segment, and the ledger swapped to solved.
  expect(await storedEntry(page)).toBe("v1.1A.2A.3A|s");
});

test("a board that arrives already solved is recorded as solved", async ({ page }) => {
  // The hash encodes the full correct board, so nothing here is the player's own
  // doing — this is the arrival the completion check exists to catch.
  await page.goto(`/${DAY_ONE}/1#1A.2A.3A`);

  await expect(page.getByText(s.puzzle.solved)).toBeVisible();
  await expect(async () => {
    expect(await storedEntry(page)).toMatch(/\|s$/);
  }).toPass();
});
