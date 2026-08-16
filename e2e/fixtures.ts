import { test as base, expect, type Page, type Locator } from "@playwright/test";
import { t } from "../src/i18n/index.ts";

/**
 * 2026-04-19 is `START_DATE`: day 1, the first date the shipped corpus covers,
 * and safely in the past so `isValidDate` accepts it without a debug flag.
 */
export const DAY_ONE = "2026-04-19";
export const DAY_ONE_L1 = `/${DAY_ONE}/1`;

/**
 * The level-1 board is 3 questions × 3 options and its one solution is all-A.
 * From `cargo run -- check public/puzzles/daily/2026.json 0419-1 --json`
 * (`brute_solutions`). Re-derive it there if that corpus entry ever changes.
 */
export const DAY_ONE_L1_SOLUTION = [0, 0, 0];

export const s = t();

/**
 * One cell, addressed the way the app's own keyboard navigation addresses it —
 * question index and option index, both 0-based.
 */
export function cell(page: Page, questionIndex: number, optionIndex: number): Locator {
  return page.locator(`[data-qi="${questionIndex}"][data-oi="${optionIndex}"]`);
}

/** A click eliminates, a second marks correct, a third clears. */
export async function markCorrect(page: Page, questionIndex: number, optionIndex: number) {
  const target = cell(page, questionIndex, optionIndex);
  await target.click();
  await target.click();
}

/** Plays the day-one level-1 board through to its solution. */
export async function solveDayOneL1(page: Page) {
  for (const [questionIndex, optionIndex] of DAY_ONE_L1_SOLUTION.entries()) {
    await markCorrect(page, questionIndex, optionIndex);
  }
}

/**
 * Seeds the keys that would otherwise let first-visit state vary between runs,
 * before any app code executes.
 */
export const test = base.extend({
  page: async ({ page }, use) => {
    await page.addInitScript(() => {
      // Otherwise the how-to-play block auto-expands and reflows the page mid-test.
      localStorage.setItem("refpuzzle:onboarded", "1");
      localStorage.setItem("refpuzzle:theme", "light");
    });
    await use(page);
  },
});

export { expect };
