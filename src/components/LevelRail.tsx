import { classNames } from "../lib/classNames.ts";
import type { PuzzleProgress } from "../lib/store.ts";
import { levelProgress } from "../puzzles/progress.ts";

/**
 * One day's six levels as a rail, thicker the further along each is. A stale
 * day grays every level but the ones needing a recheck. Hidden from screen
 * readers: the host's own label has to state the same progress in words.
 */
export function LevelRail({ states }: { states: PuzzleProgress[] }) {
  const stale = states.some((state) => state.stale);
  return (
    <span class={classNames("level-rail", stale && "stale")} aria-hidden="true">
      {states.map((state, i) => (
        // oxlint-disable-next-line react/no-array-index-key
        <span key={i} class={classNames("level-seg", levelProgress(state))} />
      ))}
    </span>
  );
}
