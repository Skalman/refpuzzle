import { useEffect, useRef } from "preact/hooks";
import { IconCheck, IconDot, IconWarning } from "./Icons.tsx";
import { LEVELS, puzzleId } from "../puzzles/daily.ts";
import { hasState } from "../lib/store.ts";
import { levelProgress, type LevelProgress } from "../puzzles/progress.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import { LEVEL_COLOR } from "./ui/styles.ts";
import { t } from "../i18n/index.ts";
import { tw } from "../lib/classNames.ts";

/** The slot the status icon sits in, ahead of the level's name. */
const TAB_ICON = tw`mr-[0.25em] inline-flex align-middle`;

/** A tab: underlined in its level's color, raised while selected. */
const TAB = tw`flex-[1_0_auto] cursor-pointer rounded-t-md border-0 border-b-2 border-solid border-b-[color-mix(in_srgb,var(--level-color)_75%,transparent)] bg-transparent px-2.5 py-1.5 text-center text-chrome whitespace-nowrap text-muted transition-all duration-150 first:rounded-bl-md last:rounded-br-md hover:bg-hover aria-selected:border-b-(--level-color) aria-selected:bg-surface aria-selected:font-semibold aria-selected:text-(--level-color) aria-selected:shadow-raised md:flex-1`;

/** The icon ahead of a level's name: needing a recheck, solved, or begun. */
function TabStatus({ progress }: { progress: LevelProgress }) {
  switch (progress) {
    case "stale":
      return (
        <span class={`${TAB_ICON} text-invalid`}>
          <IconWarning size="0.9em" />{" "}
        </span>
      );
    case "solved":
      return (
        <span class={`${TAB_ICON} text-valid`}>
          <IconCheck size="0.9em" class="stroke-3 group-aria-selected:stroke-4" />{" "}
        </span>
      );
    case "started":
      return (
        <span class={`${TAB_ICON} text-accent group-aria-selected:scale-140`}>
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
      class="mb-2 flex gap-0 overflow-x-auto rounded-lg bg-hover p-0.5 scrollbar-none [&::-webkit-scrollbar]:hidden"
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
          class={`group ${TAB} ${LEVEL_COLOR[i]}`}
          onClick={() => onSelect(level)}
        >
          <TabStatus progress={progress[i]} />
          <span class="text-chrome">{s.difficulty[level]}</span>
        </button>
      ))}
    </div>
  );
}
