import { test, expect, markCorrect, s, DAY_ONE_L1 } from "./fixtures.ts";

test("the hint button reveals a step", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  const hintPanel = page.locator(".puzzle-hint");
  await expect(hintPanel).toBeHidden();

  await page.getByRole("button", { name: s.puzzle.hint }).click();

  // The prose itself comes from Rust, so assert that a step showed up, not its wording.
  await expect(hintPanel).toBeVisible();
  await expect(hintPanel).not.toBeEmpty();
});

test("More walks the mistake ladder from vague to the specific option", async ({ page }) => {
  await page.goto(DAY_ONE_L1);

  // The solution is all-A, so answering B to #1 is a key-wrong mark. That routes
  // the hint through the mistake ladder, whose three rungs are frontend copy.
  await markCorrect(page, 0, 1);

  const hintPanel = page.locator(".puzzle-hint");
  // The header menu and the share split-button both also spell "More".
  const more = hintPanel.getByRole("button", { name: s.puzzle.more });

  await page.getByRole("button", { name: s.puzzle.hint }).click();
  await expect(hintPanel).toContainText(s.mistake.vague);

  await more.click();
  await expect(hintPanel).toContainText(s.mistake.question(0));

  await more.click();
  await expect(hintPanel).toContainText(s.mistake.answer(0, "B"));

  // The ladder is exhausted, so there is nothing left to disclose.
  await expect(more).toBeHidden();
});
