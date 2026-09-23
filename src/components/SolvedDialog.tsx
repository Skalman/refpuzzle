import { useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { confetti } from "../lib/confetti.ts";
import { useShareable } from "../lib/hooks.ts";
import { hostOf } from "../lib/share.ts";
import { Modal } from "./Modal.tsx";
import { dayNumber } from "../puzzles/daily.ts";
import { classNames } from "../lib/classNames.ts";
import type { QuestionOutcome, SolveStats } from "../lib/solve-summary.ts";
import { IconAlert, IconClock, IconHint, IconPin, IconShare, IconUndo } from "./Icons.tsx";
import { Brand } from "./Brand.tsx";

/** Colors in the rainbow, red to purple; matches the `.rainbow-N` rules and the emoji row. */
const RAINBOW_COLORS = 6;

/**
 * Rainbow color indices for a perfect row of `count` squares: a hand-picked
 * subset up to five, the full cycle from six.
 */
function rainbowIndices(count: number): number[] {
  const subsets: Record<number, number[]> = {
    1: [3],
    2: [0, 4],
    3: [0, 2, 4],
    4: [0, 2, 3, 4],
    5: [0, 2, 3, 4, 5],
  };
  return subsets[count] ?? Array.from({ length: count }, (_x, qi) => qi % RAINBOW_COLORS);
}

/** `m:ss`, or `h:mm:ss` past the hour; null passes through. */
function formatDuration(totalS: number | null): string | null {
  if (totalS === null) return null;
  const hours = Math.floor(totalS / 3600);
  const minutes = Math.floor((totalS % 3600) / 60);
  const seconds = totalS % 60;
  const mm = hours ? String(minutes).padStart(2, "0") : String(minutes);
  return `${hours ? `${hours}:` : ""}${mm}:${String(seconds).padStart(2, "0")}`;
}

/**
 * The shareable picture of a solve: level, day, time, and one square per
 * question colored by how it went.
 */
function ResultCard({
  dateStr,
  level,
  time,
  outcomes,
  rainbow,
  host,
}: {
  dateStr: string;
  level: number;
  /** Null when the solve was never timed here; the card then skips it. */
  time: string | null;
  outcomes: QuestionOutcome[];
  /** The colors a perfect row runs, or null when the row reads square by square. */
  rainbow: number[] | null;
  host: string;
}) {
  const s = t();
  return (
    <div class="result-card" aria-hidden="true">
      <div class="result-card-top">
        <span class="result-card-brand">
          <Brand />
        </span>
        <span>{s.daily.dayLabel(dayNumber(dateStr), dateStr)}</span>
      </div>
      <div class="result-card-level">{s.difficulty[level]}</div>
      {time && (
        <>
          <div class="result-card-caption">{s.puzzle.solvedIn}</div>
          <div class="result-card-time">{time}</div>
        </>
      )}
      {/* A perfect row runs the rainbow instead, led by its emoji. */}
      <div class="result-card-squares">
        {rainbow && <span class="result-card-rainbow">{s.share.perfectEmoji}</span>}
        {outcomes.map((outcome, qi) => (
          <span
            // oxlint-disable-next-line react/no-array-index-key
            key={qi}
            class={classNames("result-card-square", rainbow ? `rainbow-${rainbow[qi]}` : outcome)}
          />
        ))}
      </div>
      <div class="result-card-host">{host}</div>
    </div>
  );
}

/** One summary line: a label, then what earned it when there is more to say. */
type SolvedLine = { icon: ComponentChildren; label: string; detail?: string };

/**
 * The solve's summary: stats, the result to share, then the ways onward.
 * Opened on the solve itself it celebrates, with confetti over the dialog;
 * reopened later from the completion bar it just reports.
 */
export function SolvedDialog({
  stats,
  dateStr,
  level,
  outcomes,
  hasNext,
  shareUrl,
  celebrate,
  onNext,
  onClose,
}: {
  stats: SolveStats;
  dateStr: string;
  level: number;
  outcomes: QuestionOutcome[];
  hasNext: boolean;
  shareUrl: string;
  celebrate: boolean;
  onNext: () => void;
  onClose: () => void;
}) {
  const s = t();

  // The shell opens the dialog in its own effect, which runs first, so the
  // confetti lands in the top layer above it.
  useEffect(() => (celebrate ? confetti() : undefined), [celebrate]);

  const time = formatDuration(stats.elapsedS);
  const perfect = outcomes.every((outcome) => outcome === "clean");
  const rainbow = perfect ? rainbowIndices(outcomes.length) : null;
  const squares = rainbow
    ? `${s.share.perfectEmoji} ${rainbow.map((index) => s.share.rainbowEmoji[index]).join("")}`
    : outcomes.map((outcome) => s.share.outcomeEmoji[outcome]).join("");
  const shareable = useShareable({
    text: [
      s.share.resultHeadline(dayNumber(dateStr), s.difficulty[level], time),
      squares,
      shareUrl,
    ].join("\n"),
  });

  // One line per measure — time, hints, checkpoints, rewinds — leaving out
  // the measures this solve can't know.
  const { solvedLines } = s.puzzle;
  const pressed = stats.checkpoints + stats.checkpointFails;
  const lines: SolvedLine[] = [];
  if (time) lines.push({ icon: <IconClock />, ...solvedLines.solvedIn(time) });
  lines.push({
    icon: <IconHint class="icon-hint" />,
    ...(stats.hints ? solvedLines.peeker(stats.hints) : solvedLines.pathfinder),
  });
  lines.push(
    stats.checkpointFails
      ? {
          icon: <IconAlert strokeWidth={3} class="icon-error" />,
          ...solvedLines.oopsie(stats.checkpointFails, pressed),
        }
      : {
          icon: <IconPin class="icon-checkpoint" />,
          ...(stats.checkpoints
            ? solvedLines.doubleChecker(stats.checkpoints)
            : solvedLines.freeSpirit),
        },
  );
  if (stats.historyBursts !== null) {
    lines.push({
      icon: <IconUndo />,
      ...(stats.historyBursts
        ? solvedLines.timeTraveler(stats.historyBursts)
        : solvedLines.straightSolver),
    });
  }

  return (
    <Modal
      title={perfect ? s.puzzle.solvedPerfect : s.puzzle.solved}
      class="solved-dialog"
      onClose={onClose}
    >
      <ul class="solved-lines">
        {lines.map((line) => (
          <li key={line.label}>
            <span class="solved-line-icon">{line.icon}</span>
            <span>
              <strong>{line.label}</strong>
              {line.detail && ` — ${line.detail}`}
            </span>
          </li>
        ))}
      </ul>
      <ResultCard
        dateStr={dateStr}
        level={level}
        time={time}
        outcomes={outcomes}
        rainbow={rainbow}
        host={hostOf(shareUrl)}
      />
      <div class="solved-share">
        {shareable.canShare && (
          <button class="outline-btn" onClick={shareable.share}>
            <IconShare size="0.9em" /> {s.share.share}
          </button>
        )}
        <button class="outline-btn" onClick={shareable.copy}>
          {shareable.copied ? s.share.copied : s.share.copyText}
        </button>
      </div>
      <div class="solved-actions">
        {hasNext && (
          <button class="next-puzzle-btn solved-primary" onClick={onNext} autofocus>
            {s.puzzle.nextPuzzle} &rarr;
          </button>
        )}
        {hasNext ? (
          <a href="/archive" class="toolbar-accent-btn">
            {s.daily.archive}
          </a>
        ) : (
          <a href="/archive" class="primary-btn solved-primary" autofocus>
            {s.daily.archive} &rarr;
          </a>
        )}
      </div>
    </Modal>
  );
}
