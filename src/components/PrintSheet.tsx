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
    <div class="hidden print:visible print:absolute print:top-0 print:left-0 print:block print:w-full print:bg-[#fff] print:p-4 print:text-[11pt] print:text-[#000] print:**:visible">
      <h1 class="font-bold print:mb-2 print:border-b print:border-[#000] print:pb-[0.3rem] print:text-[14pt]">
        {s.app.title} &mdash; {s.daily.dayLabel(dayNumber(dateStr), dateStr)}
      </h1>
      {LEVELS.map((level) => {
        const p = puzzles[`${level}`];
        if (!p) return null;
        return (
          <div key={level} class="print:mb-4 print:break-inside-avoid">
            <h2 class="font-bold print:mb-[0.3rem] print:text-[12pt]">
              {s.difficulty[level]} ({p.questions.length} {s.puzzleList.questions})
            </h2>
            {p.questions.map((q, qi) => (
              <div key={q.text} class="print:mb-[0.4rem]">
                <div class="print:mb-[0.15rem] print:font-semibold">
                  {qi + 1}. {q.text}
                </div>
                <div
                  class={classNames(
                    "print:flex print:flex-wrap print:pl-[1.2em]",
                    q.options.some((l) => l.length > 12) && "print:flex-col print:gap-[0.1em]",
                  )}
                >
                  {q.options.map((label, oi) => (
                    <span key={LETTERS[oi]} class="print:min-w-[5em] print:flex-1">
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
