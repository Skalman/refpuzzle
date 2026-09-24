import { test, expect, cell, markCorrect, s, DAY_ONE, DAY_ONE_L1 } from "./fixtures.ts";

/** The dialog prints the URL with the protocol stripped and no trailing slash. */
async function dialogUrl(page: import("@playwright/test").Page): Promise<string> {
  const shown = await page.getByTestId("share-url").innerText();
  return `http://${shown}`;
}

/** The dialog lives behind the header's More menu and opens on Puzzle. */
async function openShareDialog(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: s.aria.more }).click();
  await page.getByRole("menuitem", { name: s.share.share }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
}

test("the share dialog offers the puzzle's own URL", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await openShareDialog(page);

  expect(await dialogUrl(page)).toMatch(new RegExp(`/${DAY_ONE}/1$`));
});

test("a shared progress URL restores the board on a clean device", async ({ page, browser }) => {
  await page.goto(DAY_ONE_L1);

  await markCorrect(page, 0, 0);
  await cell(page, 1, 1).click();

  await openShareDialog(page);
  await page.getByRole("radio", { name: s.share.modes.progress }).check();
  const url = await dialogUrl(page);

  // The hash has to carry the marks on its own, so open it with nothing stored.
  const fresh = await browser.newContext();
  const freshPage = await fresh.newPage();
  await freshPage.addInitScript(() => {
    localStorage.setItem("refpuzzle:onboarded", "1");
    localStorage.setItem("refpuzzle:theme", "light");
  });
  await freshPage.goto(url);

  await expect(freshPage.locator('[data-qi="0"][data-oi="0"]')).toHaveAttribute(
    "data-mark",
    "correct",
  );
  await expect(freshPage.locator('[data-qi="1"][data-oi="1"]')).toHaveAttribute(
    "data-mark",
    "incorrect",
  );

  await fresh.close();
});

/** A solved day-one level 1 board whose track carries one hint marker. */
const SOLVED_WITH_HINT = `/${DAY_ONE}/1#v1.h1q1.1A.2A.3A`;
/** The same, one mark in and unsolved. */
const STARTED_WITH_HINT = `/${DAY_ONE}/1#v1.h1q1.1A`;

const ENTRY = "refpuzzle:puzzle:/2026-04-19/1";

test("a shared solved board is recorded without the sharer's markers", async ({ page }) => {
  await page.goto(SOLVED_WITH_HINT);

  // The marker rides the link, but it records someone else's hint, so the board
  // is adopted without it.
  await expect(page.locator('[data-qi="0"][data-oi="0"]')).toHaveAttribute("data-mark", "correct");
  await expect(page.getByTestId("history-hint")).toHaveCount(0);

  // Arrival records the solve. Read the entry rather than the board, which
  // would re-decode the hash the reload carries along.
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), ENTRY)).toContain("1A");
  const stored = await page.evaluate((key) => localStorage.getItem(key), ENTRY);
  expect(stored).not.toMatch(/h\d/);
});

test("playing on from a shared progress link keeps the sharer's markers out", async ({ page }) => {
  await page.goto(STARTED_WITH_HINT);

  await expect(page.locator('[data-qi="0"][data-oi="0"]')).toHaveAttribute("data-mark", "correct");
  await expect(page.getByTestId("history-hint")).toHaveCount(0);

  // The first mark of this device's own is what writes the entry.
  await markCorrect(page, 1, 1);

  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), ENTRY)).toContain("2B");
  const stored = await page.evaluate((key) => localStorage.getItem(key), ENTRY);
  expect(stored).not.toMatch(/h\d/);
});
