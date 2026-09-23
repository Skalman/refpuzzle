import { useEffect, useRef, useState } from "preact/hooks";
import type { Answer, Puzzle } from "../engine/types.ts";
import { letterIdx } from "../engine/types.ts";
import { deriveState } from "../engine/state.ts";
import type { DeduceAction, ExplainStep, SolveStep } from "../engine/hint-types.ts";
import { findMistake } from "../engine/mistake.ts";
import type { Mistake } from "../engine/mistake.ts";
import type { ArrowCell, ArrowReferent, ArrowSpec, CoachMessage } from "../engine/coach-types.ts";
import type { QuestionState } from "../lib/store.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";
import { pointerKind } from "../lib/pointer.ts";
import { t, qList } from "../i18n/index.ts";

/** The marking-gesture reminder in the wording that matches the pointer. */
function markingGesture(): string {
  return t().coach.markingGesture(pointerKind());
}

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
   * the depth (1 = the opening line, 2 = the sharpened/revealing follow-up).
   */
  onHint?: (level: number, qi: number | null) => void;
}

type EngineState = { answers: (Answer | null)[]; eliminated: number[] };

/**
 * The cells a step's mark lands on, question order: one for a force or a single
 * elimination, one per question × option for a batch one.
 */
function actionCells(a: DeduceAction): ArrowCell[] {
  if (a.type === "force") return [{ qi: a.qi, oi: letterIdx(a.answer) }];
  if (a.type === "eliminate") return [{ qi: a.qi, oi: a.oi }];
  const cells: ArrowCell[] = [];
  for (let qi = 0; qi < 12; qi++) {
    if (!((a.questionMask >> qi) & 1)) continue;
    for (let oi = 0; oi < 5; oi++) {
      if ((a.optionMask >> oi) & 1) cells.push({ qi, oi });
    }
  }
  return cells;
}

/** The question a step marks first — the lowest in a batch elimination's mask. */
function markedQuestion(a: DeduceAction): number {
  return actionCells(a)[0]?.qi ?? 0;
}

/**
 * The question a message's arrow anchors on — what its hint marker records,
 * and null for a message that draws none. `settles` and `connector` arrows run
 * *from* the question whose meaning fires the step, so that end anchors them,
 * not what the arrow reaches.
 */
function anchorQuestion(message: CoachMessage): number | null {
  const { arrow } = message;
  if (!arrow) return null;
  return arrow.mode === "point" ? (arrow.qis[0] ?? null) : arrow.qi;
}

/**
 * The question the explanation opens on, when it opens on exactly one — the one
 * whose meaning fires the rule, so its referent is the relationship the step
 * teaches. A force/elimination opens on the question it marks; a batch
 * elimination opens on the question doing the constraining, which is *not*
 * among the questions the mark lands on. Null when several open it — an arrow
 * anchors on one question or on none.
 */
function leadQuestion(steps: ExplainStep[]): number | null {
  for (const step of steps) {
    if (step.type === "look") return step.qis.length === 1 ? step.qis[0] : null;
  }
  return null;
}

/**
 * The questions the coach points/starts at: those the explanation reads (from
 * the engine), which may differ from where the mark lands; falls back to the
 * question marked. Always non-empty.
 */
function stepFocus(step: SolveStep): number[] {
  return step.focusQis.length > 0 ? step.focusQis : [markedQuestion(step.action)];
}

/**
 * The gentle "start here" pointer for a step — names and points at every
 * question it reads, wording the available move as pin-down (force) vs.
 * eliminate. Shared by the intro where-to-start and the ~10s orient line.
 */
function whereToStart(step: SolveStep): CoachMessage {
  const s = t().coach;
  const focus = stepFocus(step);
  const list = qList(focus);
  const text = step.action.type === "force" ? s.lookForce(list) : s.lookEliminate(list);
  return {
    text,
    arrow: { mode: "point", qis: focus },
    tone: "calm",
  };
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
 * line), the move the guided line offers to walk.
 */
function explainLine(steps: ExplainStep[]): string {
  if (steps.length === 0) return "";
  const last = steps[steps.length - 1];
  if (last.type === "complex") return last.header;
  if (last.type === "look") return t().hint.tryLooking(last.qis);
  return last.text;
}

/**
 * The calm line the coach rests on while solving with nothing specific to say,
 * so the reserved space never sits empty.
 */
function buildResting(): CoachMessage {
  return { text: markingGesture(), arrow: null, tone: "calm" };
}

/**
 * The `idx`-th line of the intro cycle: the mental model, then the marking
 * gesture, then where to start — the blank board's first move, the same step
 * the Hint button surfaces first, pointed at the question it leads with. Never
 * says why.
 */
function buildIntro(puzzle: Puzzle, handle: PuzzleHandle | null, idx: number): CoachMessage {
  const s = t().coach;
  if (idx === 0) return { text: s.mentalModel, arrow: null, tone: "calm" };
  if (idx === 1) return { text: markingGesture(), arrow: null, tone: "calm" };
  const blank = blankState(puzzle);
  const step = handle?.nextStep(blank.answers, blank.eliminated);
  return step ? whereToStart(step) : { text: s.lookGeneric, arrow: null, tone: "calm" };
}

/**
 * The L1 in-play coach. Ambient teaching that only speaks when the newcomer is
 * stuck or wandering and falls silent the instant they engage; an expert who
 * starts marking never sees past the first intro line. Reuses the solver
 * (`solve` for the answer key, `nextStep` for where-to-start / guidance) — nothing
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
    // Anchor on the question the explanation opens on — the one whose meaning
    // fires the step, not `focus[0]` (the lowest-indexed question read). Where
    // its conclusion lands decides what the arrow draws: cells on *other*
    // questions mean it's doing something to them, so draw that; a step that
    // only marks the asking question is instead explained by what that question
    // reads, i.e. its referent.
    const anchorQi = leadQuestion(step.explain) ?? markedQuestion(step.action);
    const settled = actionCells(step.action).filter((c) => c.qi !== anchorQi);
    const referent = getReferents()?.[anchorQi] ?? null;
    let arrow: ArrowSpec = { mode: "point", qis: focus };
    if (settled.length > 0) arrow = { mode: "settles", qi: anchorQi, cells: settled };
    else if (referent) arrow = { mode: "connector", qi: anchorQi, referent };
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
  function buildMistakePoint(mistake: Mistake): CoachMessage {
    return {
      text: t().coach.mistakeElim(mistake.qi + 1),
      arrow: { mode: "point", qis: [mistake.qi], oi: letterIdx(mistake.answer) },
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
  // drops back to the resting line; specific lines/mistakes replace it only
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
          const note = buildMistake(still);
          setMessage(note);
          onHintRef.current?.(1, anchorQuestion(note));
        }
      }, MISTAKE_MS);
      // Still stuck on an elimination a while later → sharpen the halo to the
      // wrongly ruled-out option (wrong answers are already visible; no escalation).
      if (mistake.kind === "elim") {
        timers.current.mistakePoint = window.setTimeout(() => {
          const now = deriveMarks(stateRef.current.questions, optionCount);
          const still = findMistake(now.answers, now.eliminated, solution);
          if (still && still.kind === "elim" && !stateRef.current.completed) {
            const note = buildMistakePoint(still);
            setMessage(note);
            onHintRef.current?.(2, anchorQuestion(note));
          }
        }, MISTAKE_MS + MISTAKE_POINT_MS);
      }
    } else {
      timers.current.orient = window.setTimeout(() => {
        const msg = stateRef.current.completed ? null : buildOrient();
        if (msg) {
          setMessage(msg);
          onHintRef.current?.(1, anchorQuestion(msg));
        }
      }, IDLE_ORIENT_MS);
      timers.current.guide = window.setTimeout(() => {
        const msg = stateRef.current.completed ? null : buildGuide();
        if (msg) {
          setMessage(msg);
          onHintRef.current?.(2, anchorQuestion(msg));
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
      setMessage(buildIntro(puzzle, handleRef.current, idx));
      idx = (idx + 1) % 3;
      timer = window.setTimeout(tick, INTRO_CYCLE_MS);
    };
    tick();
    return () => window.clearTimeout(timer);
  }, [enabled, started, completed, puzzle, handleRef]);

  // Re-evaluate (and reset the idle/mistake timers) only when the board actually
  // changes — a mark/undo/redo/reset — plus phase/handle. Bare clicks and key
  // nav leave a relevant hint in place rather than snapping back to resting.
  useEffect(() => {
    evaluateRef.current();
    return clearSolvingTimers;
  }, [questions, started, completed, enabled, handleReady]);

  return { message: enabled ? message : null };
}
