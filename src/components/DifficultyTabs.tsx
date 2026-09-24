import { useEffect, useRef } from "preact/hooks";
import { IconCheck, IconDot, IconWarning } from "./Icons.tsx";
import { LEVELS, puzzleId } from "../puzzles/daily.ts";
import { hasState } from "../lib/store.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import { classNames } from "../lib/classNames.ts";
import { t } from "../i18n/index.ts";

/** The icon ahead of a level's name: needing a recheck, solved, or begun. */
function TabStatus({
  started,
  solved,
  stale,
}: {
  started: boolean;
  solved: boolean;
  stale: boolean;
}) {
  if (stale) {
    return (
      <span class="tab-stale-icon">
        <IconWarning size="0.9em" />{" "}
      </span>
    );
  }
  if (solved) {
    return (
      <span class="tab-check">
        <IconCheck size="0.9em" />{" "}
      </span>
    );
  }
  if (started) {
    return (
      <span class="tab-started-dot">
        <IconDot size="0.9em" />{" "}
      </span>
    );
  }
  return null;
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

  const activeTabState = hasState(puzzleId(dateStr, activeLevel));
  const activeTabIcon = activeTabState.stale
    ? "stale"
    : activeTabState.completed
      ? "solved"
      : activeTabState.started
        ? "started"
        : "";

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
  }, [activeLevel, activeTabIcon]);

  return (
    <div
      ref={tabsRef}
      class="difficulty-tabs"
      role="tablist"
      onKeyDown={arrowNavHandler(".difficulty-tab")}
    >
      {LEVELS.map((level) => {
        const { started, completed: solved, stale } = hasState(puzzleId(dateStr, level));
        return (
          <button
            key={level}
            role="tab"
            aria-selected={activeLevel === level}
            tabIndex={activeLevel === level ? 0 : -1}
            class={classNames(
              "difficulty-tab",
              activeLevel === level && "active",
              solved && !stale && "tab-solved",
              stale && "tab-stale",
              started && "tab-started",
            )}
            onClick={() => onSelect(level)}
          >
            <TabStatus started={started} solved={solved} stale={stale} />
            <span class="tab-label">{s.difficulty[level]}</span>
          </button>
        );
      })}
    </div>
  );
}
