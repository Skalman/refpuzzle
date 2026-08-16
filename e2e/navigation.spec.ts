import { test, expect, s, DAY_ONE, DAY_ONE_L1 } from "./fixtures.ts";

test("the level tabs switch puzzles and the URL follows", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await page.getByRole("tab", { name: s.difficulty[3] }).click();

  await expect(page).toHaveURL(new RegExp(`/${DAY_ONE}/3$`));
  await expect(page.getByRole("tab", { name: s.difficulty[3] })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("the archive links through to a day", async ({ page }) => {
  await page.goto("/archive");

  await expect(page.getByRole("heading", { name: s.daily.archive })).toBeVisible();

  // The day's accessible name is a locale-formatted date, so address it by the
  // href — that's the navigation contract the grid actually promises.
  await page.locator(`a[href="/${DAY_ONE}/1"]`).click();

  await expect(page).toHaveURL(new RegExp(`/${DAY_ONE}/1$`));
});

test("the old /past slug redirects to the archive", async ({ page }) => {
  await page.goto("/past");

  await expect(page).toHaveURL(/\/archive$/);
  await expect(page.getByRole("heading", { name: s.daily.archive })).toBeVisible();
});

test("an out-of-range date is refused", async ({ page }) => {
  // Before START_DATE, so isValidDate rejects it.
  await page.goto("/2020-01-01/1");

  await expect(page.getByRole("heading", { name: s.notFound.noPuzzle })).toBeVisible();
});

test("an unknown route shows the 404", async ({ page }) => {
  await page.goto("/no-such-page");

  await expect(page.getByRole("heading", { name: s.notFound.title })).toBeVisible();
  await expect(page.getByText(s.notFound.pageNotFound)).toBeVisible();
});
