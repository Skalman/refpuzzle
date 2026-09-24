import { test, expect, s, solveDayOneL1, DAY_ONE, DAY_ONE_L1 } from "./fixtures.ts";

/** The stored entry for day-one level 1, ledger included. */
function storedEntry(page: import("@playwright/test").Page) {
  return page.evaluate(() => localStorage.getItem("refpuzzle:puzzle:/2026-04-19/1"));
}

/**
 * The completion bar. The celebration dialog carries the same two buttons while
 * it is up, so the tests below say which they mean.
 */
function completionBar(page: import("@playwright/test").Page) {
  return page.getByTestId("completion-bar");
}

test("solving the board shows the completion bar", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await expect(completionBar(page)).toBeHidden();

  await solveDayOneL1(page);

  await expect(completionBar(page)).toBeVisible();
  // Level 1 of 6, so the bar offers the next level rather than the archive.
  await expect(
    completionBar(page).getByRole("button", { name: new RegExp(s.puzzle.nextPuzzle) }),
  ).toBeVisible();
});

test("the next-puzzle button moves to level 2", async ({ page }) => {
  await page.goto(DAY_ONE_L1);
  await solveDayOneL1(page);

  // The celebration dialog is up and carries the same button; it is the one a
  // player reaches first.
  await page
    .getByRole("dialog")
    .getByRole("button", { name: new RegExp(s.puzzle.nextPuzzle) })
    .click();

  await expect(page.getByRole("tab", { name: s.difficulty[2] })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page).toHaveURL(/\/2026-04-19\/2$/);
});

test("a solved board is still solved after a reload", async ({ page }) => {
  await page.goto(DAY_ONE_L1);
  await solveDayOneL1(page);
  await expect(completionBar(page)).toBeVisible();

  await page.reload();

  await expect(completionBar(page)).toBeVisible();
  // The level tab reports the solve too.
  await expect(page.getByRole("tab", { name: s.difficulty[1] })).toHaveAttribute(
    "data-progress",
    "solved",
  );
});

test("solving keeps the history the player built", async ({ page }) => {
  await page.goto(DAY_ONE_L1);
  await solveDayOneL1(page);
  await expect(completionBar(page)).toBeVisible();

  // Every mark still in the segment, and the ledger's solved flag set. The
  // counters follow the flags, so match the flags rather than the tail.
  expect(await storedEntry(page)).toMatch(/^v1\.1A\.2A\.3A\|s(\.|$)/);
});

test("a board that arrives already solved is recorded as solved", async ({ page }) => {
  // The hash encodes the full correct board, so nothing here is the player's own
  // doing — this is the arrival the completion check exists to catch.
  await page.goto(`/${DAY_ONE}/1#1A.2A.3A`);

  await expect(completionBar(page)).toBeVisible();
  await expect(async () => {
    expect(await storedEntry(page)).toMatch(/\|s(\.|$)/);
  }).toPass();
});
