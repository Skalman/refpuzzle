import { classNames, tw } from "../lib/classNames.ts";
import type { PuzzleProgress } from "../lib/store.ts";
import { levelProgress, type LevelProgress } from "../puzzles/progress.ts";
import { LEVEL_COLOR } from "./ui/styles.ts";

/**
 * A segment's thickness and ink by progress. Whole pixels, not rem, so the
 * steps stay exact at every root size.
 */
const SEGMENT: Record<NonNullable<LevelProgress> | "untouched", string> = {
  untouched: tw`h-px bg-[color-mix(in_srgb,var(--level-ink)_70%,transparent)]`,
  started: tw`h-(--level-started) bg-(--level-ink)`,
  solved: tw`h-(--level-solved) bg-(--level-ink)`,
  stale: tw`h-(--level-solved) bg-(--level-ink)`,
};

/**
 * One day's six levels as a rail, thicker the further along each is. A stale
 * day grays every level but the ones needing a recheck. Hidden from screen
 * readers: the host's own label has to state the same progress in words.
 *
 * Six fixed slots in tab order, so a missing level reads by position. The
 * height holds the thickest segment, so completing a level nudges nothing.
 * Hosts size and tint it through the --level-* properties, which default to
 * the archive's small days.
 */
export function LevelRail({
  states,
  class: extraClass,
}: {
  states: PuzzleProgress[];
  class?: string;
}) {
  const stale = states.some((state) => state.stale);
  return (
    <span
      class={classNames(
        "flex h-(--level-solved) items-center gap-px [--level-solved:5px] [--level-started:3px] [--level-strength:75%] [--level-surface:var(--bg-surface)]",
        extraClass,
      )}
      aria-hidden="true"
    >
      {states.map((state, i) => (
        <span
          // oxlint-disable-next-line react/no-array-index-key
          key={i}
          class={classNames(
            // Only the outer corners round, so the six read as one rail, not six pills.
            "flex-1 [--level-ink:color-mix(in_srgb,var(--level-color)_var(--level-strength),var(--level-surface))] first:rounded-bl-[2px] last:rounded-br-[2px]",
            stale
              ? state.stale
                ? "[--level-color:var(--invalid)]"
                : "[--level-color:var(--neutral-bar)]"
              : LEVEL_COLOR[i],
            SEGMENT[levelProgress(state) ?? "untouched"],
          )}
        />
      ))}
    </span>
  );
}
