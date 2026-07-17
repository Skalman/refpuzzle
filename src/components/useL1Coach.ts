import { useEffect, useRef, useState } from "preact/hooks";
import type { Answer, Puzzle } from "../engine/types.ts";
import { letterIdx } from "../engine/types.ts";
import { deriveState } from "../engine/state.ts";
import type { DeduceAction, ExplainStep, SolveStep } from "../engine/hint-types.ts";
import type { ArrowReferent, ArrowSpec, CoachMessage } from "../engine/coach-types.ts";
import type { QuestionState } from "../lib/store.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";
import { t } from "../i18n/index.ts";

// Timings (ms). Every idle/mistake timer resets on any interaction; starting
// values from the plan — tune in situ.
const INTRO_CYCLE_MS = 7000;
const MISTAKE_MS = 4000;
// Extra dwell after the flag before sharpening the halo to the exact bad option.
const MISTAKE_POINT_MS = 10_000;
const IDLE_ORIENT_MS = 10_000;
const IDLE_GUIDE_MS = 30_000;

interface CoachOpts {
  /** L1-only, daily-only — the coach never runs elsewhere. */
  enabled: boolean;
  handleRef: { current: PuzzleHandle | null };
  /** Flips true once the wasm handle exists, so the caches can fill. */
  handleReady: boolean;
  questions: QuestionState[];
  /** The player has made at least one mark (history advanced past blank). */
  started: boolean;
  completed: boolean;
  /**
   * Called when a solving hint is revealed, so it lands on the history track and
   * the hint count — as if the same info came from the Hint button. `level` is
   * the depth (1 = nudge, 2 = the sharpened/revealing follow-up).
   */
  onHint?: (level: number) => void;
}

type Mistake = { qi: number; kind: "answer" | "elim" };
type EngineState = { answers: (Answer | null)[]; eliminated: number[] };

/**
 * The question a deduce action targets, and whether it pins an answer (force)
 * vs. rules an option out (eliminate) — picks the "pin down" / "eliminate"
 * wording. `eliminateMulti` points at the lowest question in its mask.
 */
function actionTarget(a: DeduceAction): { qi: number; isForce: boolean } {
  if (a.type === "force") return { qi: a.qi, isForce: true };
  if (a.type === "eliminate") return { qi: a.qi, isForce: false };
  let qi = 0;
  for (let i = 0; i < 12; i++) {
    if ((a.questionMask >> i) & 1) {
      qi = i;
      break;
    }
  }
  return { qi, isForce: false };
}

/**
 * The questions the coach points/starts at: those the explanation reads (from
 * the engine), which may differ from where the mark lands; falls back to the
 * action's target. Always non-empty.
 */
function stepFocus(step: SolveStep): number[] {
  return step.focusQis.length > 0 ? step.focusQis : [actionTarget(step.action).qi];
}

/** A natural question list: "#1", "#1 and #3", "#1, #2 and #3". */
function qList(qis: number[]): string {
  const labels = qis.map((qi) => `#${qi + 1}`);
  if (labels.length <= 1) return labels[0] ?? "";
  if (labels.length === 2) return `${labels[0]} and ${labels[1]}`;
  return `${labels.slice(0, -1).join(", ")} and ${labels[labels.length - 1]}`;
}

/**
 * The gentle "start here" pointer for a step — names and points at every
 * question it reads, wording the available move as pin-down (force) vs.
 * eliminate. Shared by the intro where-to-start and the ~10s orient nudge.
 */
function whereToStart(step: SolveStep): CoachMessage {
  const s = t().coach;
  const focus = stepFocus(step);
  const list = qList(focus);
  return {
    text: actionTarget(step.action).isForce ? s.lookForce(list) : s.lookEliminate(list),
    arrow: { mode: "point", qis: focus },
    tone: "calm",
  };
}

/**
 * The first mark that disagrees with the answer key, or null. Mirrors the
 * hint engine's `findError`: a filled answer ≠ key, or the key answer ruled
 * out. Key-based, so it flags a mistake before the red validity bar would.
 */
function findMistake(
  answers: (Answer | null)[],
  eliminated: number[],
  solution: (Answer | null)[],
): Mistake | null {
  for (let qi = 0; qi < solution.length; qi++) {
    const correct = solution[qi];
    if (correct == null) continue;
    const oi = letterIdx(correct);
    if (answers[qi] != null && answers[qi] !== correct) return { qi, kind: "answer" };
    if ((eliminated[qi] >> oi) & 1) return { qi, kind: "elim" };
  }
  return null;
}

function blankState(puzzle: Puzzle): EngineState {
  const n = puzzle.questions.length;
  const phantom = 0b11111 & ~((1 << puzzle.optionCount) - 1);
  return {
    answers: new Array<Answer | null>(n).fill(null),
    eliminated: new Array<number>(n).fill(phantom),
  };
}

function deriveMarks(questions: QuestionState[], optionCount: number): EngineState {
  return deriveState(
    questions.map((q) => q.marks),
    optionCount,
  );
}

/**
 * The one-line gist of a step — its concrete conclusion (the last explain
 * line), the move the guided nudge offers to walk.
 */
function explainLine(steps: ExplainStep[]): string {
  if (steps.length === 0) return "";
  const last = steps[steps.length - 1];
  return last.type === "simple" ? last.text : last.header;
}

/**
 * The calm line the coach rests on while solving with nothing specific to say,
 * so the reserved space never sits empty.
 */
function buildResting(): CoachMessage {
  return { text: t().coach.markingGesture, arrow: null, tone: "calm" };
}

/**
 * The L1 in-play coach. Ambient teaching that only speaks when the newcomer is
 * stuck or wandering and falls silent the instant they engage; an expert who
 * starts marking never sees past the first intro line. Reuses the solver
 * (`solve` for the answer key, `nextStep` for where-to-start / nudges) — nothing
 * new in the engine. Returns the single message to render, or null.
 */
export function useL1Coach(
  puzzle: Puzzle,
  { enabled, handleRef, handleReady, questions, started, completed, onHint }: CoachOpts,
): { message: CoachMessage | null } {
  const [message, setMessage] = useState<CoachMessage | null>(null);
  const optionCount = puzzle.optionCount;

  // Latest inputs for the deferred timer callbacks (avoid stale closures).
  const stateRef = useRef({ enabled, started, completed, questions });
  stateRef.current = { enabled, started, completed, questions };
  const onHintRef = useRef(onHint);
  onHintRef.current = onHint;

  // Answer key + per-question arrow referents: computed once the handle exists.
  const solutionRef = useRef<(Answer | null)[] | null>(null);
  const referentsRef = useRef<(ArrowReferent | null)[] | null>(null);
  function getSolution(): (Answer | null)[] | null {
    if (!solutionRef.current && handleRef.current) solutionRef.current = handleRef.current.solve();
    return solutionRef.current;
  }
  function getReferents(): (ArrowReferent | null)[] | null {
    if (!referentsRef.current && handleRef.current)
      referentsRef.current = handleRef.current.referents();
    return referentsRef.current;
  }

  // ── Message builders (read the handle live; see the deferred callbacks) ──

  function buildIntro(idx: number): CoachMessage {
    const s = t().coach;
    if (idx === 0) return { text: s.mentalModel, arrow: null, tone: "calm" };
    if (idx === 1) return { text: s.markingGesture, arrow: null, tone: "calm" };
    // Where to start: the blank board's first move — the same step the Hint
    // button surfaces first, pointed at the question it leads with. Never says why.
    const bs = blankState(puzzle);
    const step = handleRef.current?.nextStep(bs.answers, bs.eliminated);
    return step ? whereToStart(step) : { text: s.lookGeneric, arrow: null, tone: "calm" };
  }

  /**
   * Idle ~10s: orient at the question they can work out next. Low commitment
   * — just a point, no reasoning.
   */
  function buildOrient(): CoachMessage | null {
    const handle = handleRef.current;
    if (!handle) return null;
    const state = deriveMarks(stateRef.current.questions, optionCount);
    const step = handle.nextStep(state.answers, state.eliminated);
    return step ? whereToStart(step) : null;
  }

  /**
   * Idle ~20s: offer to walk one move — the reasoning plus a connector arrow
   * from the question to its referent ("this refers to that").
   */
  function buildGuide(): CoachMessage | null {
    const handle = handleRef.current;
    if (!handle) return null;
    const state = deriveMarks(stateRef.current.questions, optionCount);
    const step = handle.nextStep(state.answers, state.eliminated);
    if (!step) return null;
    const s = t().coach;
    const focus = stepFocus(step);
    // Anchor the connector on the question being deduced (not `focus[0]`, the
    // lowest-indexed question *read*) so its referent is the relationship taught
    // — e.g. `AnswerOf #2` on #5 draws #5 → #2, not #2 → #2's own referent.
    const deduced = actionTarget(step.action).qi;
    const referent = getReferents()?.[deduced] ?? null;
    const arrow: ArrowSpec = referent
      ? { mode: "connector", qi: deduced, referent }
      : { mode: "point", qis: focus };
    return { lead: s.guidedLead, text: explainLine(step.explain), arrow, tone: "calm" };
  }

  /**
   * Mistake note: just flag the key-wrong mark and halo the question, no proof
   * and no redirect — "what to do next" only resumes once the error is fixed.
   */
  function buildMistake(mistake: Mistake): CoachMessage {
    const s = t().coach;
    const q = mistake.qi + 1;
    return {
      text: mistake.kind === "answer" ? s.mistakeAnswer(q) : s.mistakeElim(q),
      arrow: { mode: "point", qis: [mistake.qi] },
      tone: "alert",
      arrowKey: `mistake:${mistake.qi}`,
    };
  }

  /**
   * Escalated elimination note (still stuck after a while): same flag, but the
   * halo sharpens to the correct option they wrongly ruled out. Elimination only
   * — a wrong *answer* is already visible on the board, so there's nothing to
   * sharpen and re-animating would be noise.
   */
  function buildMistakePoint(mistake: Mistake, solution: (Answer | null)[]): CoachMessage {
    const bad = solution[mistake.qi];
    if (bad == null) return buildMistake(mistake);
    return {
      text: t().coach.mistakeElim(mistake.qi + 1),
      arrow: { mode: "point", qis: [mistake.qi], oi: letterIdx(bad) },
      tone: "alert",
      arrowKey: `mistake:${mistake.qi}`,
    };
  }

  // ── Solving-phase timers (idle escalation + mistake note) ──

  const timers = useRef<{
    mistake?: number;
    mistakePoint?: number;
    orient?: number;
    guide?: number;
  }>({});
  function clearSolvingTimers() {
    window.clearTimeout(timers.current.mistake);
    window.clearTimeout(timers.current.mistakePoint);
    window.clearTimeout(timers.current.orient);
    window.clearTimeout(timers.current.guide);
    timers.current = {};
  }

  // Re-armed on every interaction and on every mark. While engaged the coach
  // drops back to the resting line; specific nudges/mistakes replace it only
  // after their threshold elapses.
  const evaluateRef = useRef<() => void>(() => {});
  evaluateRef.current = () => {
    clearSolvingTimers();
    const cur = stateRef.current;
    if (!cur.enabled || cur.completed) {
      setMessage(null);
      return;
    }
    if (!cur.started) return; // intro phase owns the message
    const solution = getSolution();
    if (!solution) return;
    setMessage(buildResting());
    const state = deriveMarks(cur.questions, optionCount);
    const mistake = findMistake(state.answers, state.eliminated, solution);
    if (mistake) {
      timers.current.mistake = window.setTimeout(() => {
        const now = deriveMarks(stateRef.current.questions, optionCount);
        const still = findMistake(now.answers, now.eliminated, solution);
        if (still && !stateRef.current.completed) {
          setMessage(buildMistake(still));
          onHintRef.current?.(1);
        }
      }, MISTAKE_MS);
      // Still stuck on an elimination a while later → sharpen the halo to the
      // wrongly ruled-out option (wrong answers are already visible; no escalation).
      if (mistake.kind === "elim") {
        timers.current.mistakePoint = window.setTimeout(() => {
          const now = deriveMarks(stateRef.current.questions, optionCount);
          const still = findMistake(now.answers, now.eliminated, solution);
          if (still && still.kind === "elim" && !stateRef.current.completed) {
            setMessage(buildMistakePoint(still, solution));
            onHintRef.current?.(2);
          }
        }, MISTAKE_MS + MISTAKE_POINT_MS);
      }
    } else {
      timers.current.orient = window.setTimeout(() => {
        const msg = stateRef.current.completed ? null : buildOrient();
        if (msg) {
          setMessage(msg);
          onHintRef.current?.(1);
        }
      }, IDLE_ORIENT_MS);
      timers.current.guide = window.setTimeout(() => {
        const msg = stateRef.current.completed ? null : buildGuide();
        if (msg) {
          setMessage(msg);
          onHintRef.current?.(2);
        }
      }, IDLE_GUIDE_MS);
    }
  };

  // Intro sequence: calm lines cycling until the first mark. An expert who
  // marks straight away only ever glimpses the first.
  useEffect(() => {
    if (!enabled || started || completed) return undefined;
    let idx = 0;
    let timer = 0;
    const tick = () => {
      setMessage(buildIntro(idx));
      idx = (idx + 1) % 3;
      timer = window.setTimeout(tick, INTRO_CYCLE_MS);
    };
    tick();
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, started, completed, puzzle]);

  // Re-evaluate (and reset the idle/mistake timers) only when the board actually
  // changes — a mark/undo/redo/reset — plus phase/handle. Bare clicks and key
  // nav leave a relevant hint in place rather than snapping back to resting.
  useEffect(() => {
    evaluateRef.current();
    return clearSolvingTimers;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [questions, started, completed, enabled, handleReady]);

  return { message: enabled ? message : null };
}
