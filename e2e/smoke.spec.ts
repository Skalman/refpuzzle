import { test, expect, cell, s, DAY_ONE_L1 } from "./fixtures.ts";

test("the day-one board loads and its cells are markable", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  await expect(page.getByRole("tab", { name: s.difficulty[1] })).toHaveAttribute(
    "aria-selected",
    "true",
  );

  const firstCell = cell(page, 0, 0);
  await expect(firstCell).toBeVisible();

  await firstCell.click();
  await expect(firstCell).toHaveAttribute("data-mark", "incorrect");
});
