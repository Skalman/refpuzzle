import { useEffect, useRef } from "preact/hooks";
import { IconCheck, IconDot, IconWarning } from "./Icons.tsx";
import { LEVELS, puzzleId } from "../puzzles/daily.ts";
import { hasState } from "../lib/store.ts";
import { levelProgress, type LevelProgress } from "../puzzles/progress.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import { classNames } from "../lib/classNames.ts";
import { t } from "../i18n/index.ts";

/** The icon ahead of a level's name: needing a recheck, solved, or begun. */
function TabStatus({ progress }: { progress: LevelProgress }) {
  switch (progress) {
    case "stale":
      return (
        <span class="tab-stale-icon">
          <IconWarning size="0.9em" />{" "}
        </span>
      );
    case "solved":
      return (
        <span class="tab-check">
          <IconCheck size="0.9em" />{" "}
        </span>
      );
    case "started":
      return (
        <span class="tab-started-dot">
          <IconDot size="0.9em" />{" "}
        </span>
      );
    default:
      return null;
  }
}

/** One tab per level of the day, each showing how far along it is. */
export function DifficultyTabs({
  dateStr,
  activeLevel,
  onSelect,
}: {
  dateStr: string;
  activeLevel: number;
  onSelect: (level: number) => void;
}) {
  const s = t();
  const tabsRef = useRef<HTMLDivElement>(null);

  const progress = LEVELS.map((level) => levelProgress(hasState(puzzleId(dateStr, level))));
  const activeProgress = progress[activeLevel - 1];

  useEffect(() => {
    const container = tabsRef.current;
    if (!container) return;
    const tab = container.children[activeLevel - 1];
    if (!(tab instanceof HTMLElement)) return;
    // Center the tab horizontally without affecting vertical scroll (scrollIntoView would
    // also scroll the page vertically when the tab isn't fully in view).
    const tabRect = tab.getBoundingClientRect();
    const containerRect = container.getBoundingClientRect();
    const delta = tabRect.left + tabRect.width / 2 - (containerRect.left + containerRect.width / 2);
    container.scrollTo({ left: container.scrollLeft + delta, behavior: "smooth" });
  }, [activeLevel, activeProgress]);

  return (
    <div
      ref={tabsRef}
      class="difficulty-tabs"
      role="tablist"
      onKeyDown={arrowNavHandler('[role="tab"]')}
    >
      {LEVELS.map((level, i) => (
        <button
          key={level}
          role="tab"
          aria-selected={activeLevel === level}
          tabIndex={activeLevel === level ? 0 : -1}
          data-progress={progress[i] ?? undefined}
          class={classNames("difficulty-tab", activeLevel === level && "active")}
          onClick={() => onSelect(level)}
        >
          <TabStatus progress={progress[i]} />
          <span class="tab-label">{s.difficulty[level]}</span>
        </button>
      ))}
    </div>
  );
}
