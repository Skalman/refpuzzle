import type { Puzzle } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { LEVELS, dayNumber } from "../puzzles/daily.ts";
import { classNames } from "../lib/classNames.ts";
import { t } from "../i18n/index.ts";

/** Every level of the day as plain text, shown only when printing. */
export function PrintSheet({
  dateStr,
  puzzles,
}: {
  dateStr: string;
  puzzles: Record<string, Puzzle>;
}) {
  const s = t();
  return (
    <div class="print-only">
      <h1>
        {s.app.title} &mdash; {s.daily.dayLabel(dayNumber(dateStr), dateStr)}
      </h1>
      {LEVELS.map((level) => {
        const p = puzzles[`${level}`];
        if (!p) return null;
        return (
          <div key={level} class="print-puzzle">
            <h2>
              {s.difficulty[level]} ({p.questions.length} {s.puzzleList.questions})
            </h2>
            {p.questions.map((q, qi) => (
              <div key={q.text} class="print-question">
                <div class="print-question-text">
                  {qi + 1}. {q.text}
                </div>
                <div
                  class={classNames(
                    "print-options",
                    q.options.some((l) => l.length > 12) && "print-options-long",
                  )}
                >
                  {q.options.map((label, oi) => (
                    <span key={LETTERS[oi]} class="print-option">
                      {LETTERS[oi]}. {label}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}
