import { useCallback, useRef, useState, useEffect } from "preact/hooks";
import type { Answer, Puzzle } from "../engine/types.ts";
import type { ExplainStep } from "../engine/hint-types.ts";
import { deriveState } from "../engine/state.ts";
import { findMistake } from "../engine/mistake.ts";
import type { Mistake } from "../engine/mistake.ts";
import type { QuestionState } from "../lib/store.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";
import { t } from "../i18n/index.ts";

/**
 * The question a hint step names — what its hint marker records. A `look`
 * names its first question; a line that names none gives null.
 */
function namedQuestion(step: ExplainStep): number | null {
  if (step.type === "look") return step.qis[0] ?? null;
  if (step.type === "simple") return step.qi ?? null;
  return null;
}

/**
 * The hint steps for a key-wrong mark: vague, then the question, then the
 * option. `lead` opens them, so the caller can frame why they're showing.
 */
function mistakeSteps(mistake: Mistake, lead: string): ExplainStep[] {
  const s = t().mistake;
  return [
    { type: "simple", text: lead },
    { type: "simple", text: s.question(mistake.qi), qi: mistake.qi },
    {
      type: "simple",
      text:
        mistake.kind === "answer"
          ? s.answer(mistake.qi, mistake.answer)
          : s.elim(mistake.qi, mistake.answer),
      qi: mistake.qi,
    },
  ];
}

export function useHintEngine(
  puzzle: Puzzle,
  opts: {
    questionsRef: { current: QuestionState[] };
    debugMode: boolean;
    pushHintMarker: (level: number, qi: number | null) => void;
    completed: boolean;
    questions: QuestionState[];
    handleRef: { current: PuzzleHandle | null };
  },
) {
  const [hintText, setHintText] = useState<ExplainStep | null>(null);
  const hintRef = useRef<{ steps: ExplainStep[]; step: number } | null>(null);
  const [debugHints, setDebugHints] = useState<ExplainStep[] | null>(null);
  /** Whether debug steps are on screen — the value itself would loop the refresh effect below. */
  const debugShownRef = useRef(false);
  const solutionRef = useRef<(Answer | null)[] | null>(null);

  const optsRef = useRef(opts);
  optsRef.current = opts;

  const showDebugHints = useCallback((steps: ExplainStep[] | null) => {
    debugShownRef.current = steps != null;
    setDebugHints(steps);
  }, []);

  const getSolution = useCallback((): (Answer | null)[] => {
    if (!solutionRef.current) {
      const handle = optsRef.current.handleRef.current;
      if (!handle) return new Array<Answer | null>(puzzle.questions.length).fill(null);
      const t0 = performance.now();
      solutionRef.current = handle.solve();
      console.log(`solve: ${(performance.now() - t0).toFixed(1)}ms`);
    }
    return solutionRef.current;
  }, [puzzle]);

  const findError = useCallback(
    (answers: (Answer | null)[], eliminated: number[]): ExplainStep[] | null => {
      const mistake = findMistake(answers, eliminated, getSolution());
      return mistake ? mistakeSteps(mistake, t().mistake.vague) : null;
    },
    [getSolution],
  );

  const computeHint = useCallback((): ExplainStep[] | null => {
    const handle = optsRef.current.handleRef.current;
    if (!handle) return null;
    const markSets = optsRef.current.questionsRef.current.map((q) => q.marks);
    const { answers, eliminated } = deriveState(markSets, puzzle.optionCount);

    const errorSteps = findError(answers, eliminated);
    if (errorSteps) return errorSteps;

    const step = handle.nextStep(answers, eliminated);
    return step ? step.explain : null;
  }, [puzzle, findError]);

  // Debug mode leaves every step on screen; keep them current as the board
  // changes, but never conjure steps that aren't showing.
  useEffect(() => {
    if (!opts.debugMode || !debugShownRef.current || opts.completed) return;
    try {
      showDebugHints(computeHint());
    } catch (e) {
      console.error("Hint error:", e);
    }
  }, [opts.questions, opts.debugMode, opts.completed, computeHint, showDebugHints]);

  function handleHint() {
    const { debugMode, pushHintMarker } = optsRef.current;
    if (!debugMode && hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1) {
      const open = hintRef.current;
      open.step++;
      const shown = open.steps[open.step];
      setHintText(shown);
      pushHintMarker(open.step + 1, namedQuestion(shown));
      return;
    }

    let steps: ExplainStep[];
    try {
      steps = computeHint() ?? [{ type: "simple", text: t().puzzle.noNextStep }];
    } catch (e) {
      console.error("Hint error:", e);
      return;
    }
    if (debugMode) {
      showDebugHints(steps);
    } else {
      hintRef.current = { steps, step: 0 };
      setHintText(steps[0]);
      pushHintMarker(1, namedQuestion(steps[0]));
    }
  }

  /**
   * Show a mistake spotted outside the hint flow (a refused checkpoint) in the
   * hint panel, seeding the steps so "More" walks on to the question and then
   * the option. The opening line is priced by the caller, not as a hint; only
   * escalation past it lands markers.
   */
  function showMistake(mistake: Mistake, lead: string) {
    const steps = mistakeSteps(mistake, lead);
    if (optsRef.current.debugMode) {
      showDebugHints(steps);
      return;
    }
    hintRef.current = { steps, step: 0 };
    setHintText(steps[0]);
  }

  function clear() {
    setHintText(null);
    hintRef.current = null;
  }

  const hasMore =
    hintRef.current != null && hintRef.current.step < hintRef.current.steps.length - 1;

  return { hintText, debugHints, hasMore, handleHint, getSolution, showMistake, clear };
}
